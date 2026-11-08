# observed-property-kind-import

状态字段的类型名由**别的文件**声明时，参考侧靠类型检查器解析，hvolt 靠项目级类型表。

## golden 形态

`input.ets` 从 `../common/Kinds` 导入 `ImportedKind`（`export enum`）与
`ImportedAlias`（`export type = 'x' | 'y'`）。

| 字段 | 声明类型 | 参考 backstore | 行 |
| --- | --- | --- | --- |
| importedEnum | `ImportedKind` | `ObservedPropertySimplePU` | L41 |
| importedAlias | `ImportedAlias` | `ObservedPropertySimplePU` | L48 |
| importedAliasUnion | `ImportedAlias \| undefined` | `ObservedPropertySimplePU` | L55 |

第三条走的是 union 路径：`undefined` 被丢掉后剩一个幸存者 `ImportedAlias`，按单节点再判时
经项目表解析成字符串字面量联合，因此判简单。

## 机制

项目级类型表在 `src/build/ets_assemble.rs` 的预扫阶段建，与 `const enum` 表并列但分开：
那张表回答「`A.B` 渲染成什么值」，这张表回答「`X` 是什么种类」。条目来自三处，按优先级：

1. 当前文件自己的声明（`enums` / `aliases` / `classes` / `namespaces`）；
2. 项目里**导出**的 `enum` / `type` / `class` / `interface`；
3. 项目安装的第三方包声明（`<模块暂存目录>/oh_modules/<包名>/**/*.d.ets`，只读源码 import 过的包）；
4. SDK 的 ambient 声明（`<sdk>/ets/component/*.d.ts` 里的 `declare enum`，如 `FontWeight`）。

第 3 条压在 SDK 之前、项目源码之后：项目装的是这一版包，它比 SDK 里同名的东西更贴近代码。

同名不同体时整条丢弃，与本文件优先于任何外部表的顺序一致。

## 复现

```sh
scripts/oracle/new-fixture.sh observed-property-kind-import
HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh observed-property-kind-import
scripts/oracle/all.sh observed-property-kind-import    # PASS
```

`project/src/common/Kinds.ets` 是手工放的；`new-fixture.sh` 只拷 `input.ets`，不会删它。

## 包声明的 fixture 状态（布局已就绪，导入未启用）

`emit-layout.py` 已能拷贝 `project/oh_modules`（详见下一节），参考侧现在**可以**构建带包导入的工程，两种形态的读数已经拿到。`input.ets` 里的包导入仍然关闭，原因见「为什么导入还是关着」。

包布局放在 `package-layout/@demo/kinds/`（不在 `project/` 下，原因见本节末），启用时把它整目录拷成 `project/oh_modules/@demo/kinds/`：

```
oh-package.json5        { name, version, types: "Index.d.ets", dependencies: {} }
Index.d.ets             export enum DirectKind { A, B }
                        export { ReexportedKind } from './src/KindTypes'
src/KindTypes.d.ets     export enum ReexportedKind { A, B }
```

这两种形态分别对应「直接声明在被导入的文件里」与「相对再导出」。把导入放开后生成的 golden 里，两条读数都是 Simple：

```
L23  this.__direct = new ObservedPropertySimplePU(DirectKind.A, this, "direct");
L24  this.__reexported = new ObservedPropertySimplePU(ReexportedKind.B, this, "reexported");
L84  private __direct: ObservedPropertySimplePU<DirectKind>;
L91  private __reexported: ObservedPropertySimplePU<ReexportedKind>;
```

也就是说参考侧对再导出的名字同样判 Simple，「再导出会漏」这个缺口在参考侧不存在。hvolt 的输出在这四行上与 golden 一致。

**为什么导入还是关着、包布局也没放在 `project/` 下**：参考侧把包导入的说明符规范化成一个**嵌了临时布局绝对路径**的字符串：

```
golden:  import { DirectKind, ReexportedKind } from "@normalized:\0hvigor_ignore__private_var_folders_pz_…_layout_entry_oh_modules_@demo_kinds_Index.d.ets";
hvolt:   import { DirectKind, ReexportedKind } from '@demo/kinds';
```

那串路径里带着 `mktemp` 生成的目录名（`oracle-observed-property-kind-import-ll6Jfb`），每次生成 golden 都不同，因此这个 fixture 不存在稳定的 golden；留着它会一直红。要启用它，需要一个把 `hvigor_ignore` 前缀规范化掉的比较口径，而 `compare.js` 不在可改范围内。

另外，`project/oh_modules` 只要存在，`emit-layout.py` 就会把它写进生成清单的 `dependencies`，参考侧的 `modules.abc` 随之多出依赖记录，而 hvolt 没有对应记录，artifacts 那一行会永久多一条差异。所以包布局留在 `package-layout/`，等导入能稳定启用时再移进 `project/`。

原始阻断记录（已解决）：

1. 参考侧：`scripts/oracle/emit-layout.py` 只拷贝 `src/` 下的 `.ets` / `.ts` 与 `resources/`，不拷贝 `oh_modules`，写出的模块 `oh-package.json5` 里 `dependencies` 是空对象。hvigor 因此报
   `Cannot find module '@demo/kinds' or its corresponding type declarations`（`At File: …/layout/entry/src/main/ets/pages/Index.ets:2:44`），构建失败，产不出 golden。
2. hvolt 侧：单模块工程把源码暂存到临时目录（实测 `/tmp/pkgts3.…/hvolt-ets-93468/entry/pages/Index.ts`），暂存目录旁没有 `oh_modules`，`<StageModule.dir>/oh_modules` 落空，于是两个字段都输出 `ObservedPropertyObjectPU`。真工程能读到的原因是它是多模块工程，`StageModule.dir` 就是模块源码目录（`xb_components/`），该目录下自带 `oh_modules`。

启用这个 fixture 需要两处改动，都不在本轮可改范围内：`emit-layout.py` 把 `project/oh_modules` 拷进 `layout/<module>/oh_modules` 并在生成的 `oh-package.json5` 里写依赖；`ets_assemble` 的包根探测再加上 `<StageModule.dir>/../oh_modules` 这一层（对应单模块工程的 `<project>/oh_modules`）。

## 再导出的结论

`package_declarations` 会把包根下**所有** `*.d.ets` / `*.d.ts` 递归读完（深度上限 8），所以「声明在 `src/KindTypes.d.ets`、入口只写 `export { ReexportedKind } from './src/KindTypes'`」这种形态不需要额外跟随再导出：`ReexportedKind` 所在的文件本身就会被读到。真实工程里 `@linke/ui-color-picker` 的 `ColorPickerType` 正是这种结构（声明在 `src/main/ets/components/customcolorpickerdialog/CustomColorPickerDialog.d.ets`，入口只再导出），而它已经被读到并让 `Xb_SettingItem.ts` 的首个分歧后移，这是这条结论的实测依据。

## 遗留

- 第三方包这一路只读**源码直接 import 过的包**：`package_specifiers` 从模块源码里取非相对的
  `from '…'` / `import '…'`，其余包不读。这个工程实测读到 91 个声明文件（`@linke/ui-color-picker`
  9 个、`yaml` 78 个、其余 4 个），未做全量 `oh_modules` 扫描。
- 包内模块之间的相对再导出不跟随：`Index.d.ets` 里 `export { X } from './src/…'` 这种形式不入表，
  真正声明 `X` 的那个文件必须自己被读到。上面那个包恰好把 `ColorPickerType` 直接声明在被导入的
  目录里，所以能读到。
- 项目表只记录 `export` 的声明：一个模块内未导出的 `type` 不会被别的文件看到，与实际可见性一致。
