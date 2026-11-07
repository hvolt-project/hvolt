# ArkTS 编译：不经 hvigor 的自建流程

目标：hvolt 自己驱动 ArkTS 编译，只支持 Stage 模型，不支持 FA。不调用 `hvigorw`，
不经过 `@ohos/hvigor-arkts-compose` 的 `runArkPack`（它内含 FA/Stage 兼容分支）。

以下全部是在 Windows Surface（无 JDK）上实测得到的，SDK 26.0.0。

## 1. 三个入口位置

| 东西 | 位置 |
|---|---|
| 编译插件工厂 | `<SDK>/ets/build-tools/ets-loader/compile_plugin.js` 的 `sdkPlugins(cfg)` |
| rollup 本体 | `<hvigor>/node_modules/@ohos/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-base/node_modules/rollup` |
| 编译期诊断日志 | `@ohos/hvigor-arkts-base/dist/index.js` 的 `ArktsBaseLogger.getLogEvents()` |

`compile_plugin.js` 直接导出 `sdkPlugins`（函数，参数是配置）、`sdkPluginsMap`、
`initConfig`、`resolveFileExtensions`。

`sdkPlugins(cfg)` 返回 10 个插件，ArkTS 编译核心是：
`etsChecker`、`etsTransform`、`apiTransform`、`genAbc`、`createProgramPlugin`。
其余是 `memoryMonitor`、`watchChangedFiles`、`visualTransform`、`terserPlugin`、`babelPlugin`。
它们全部在 SDK 内，与 java 无关。

## 2. 流程骨架（实测可跑到编译器内部）

```js
const p = require('<SDK>/ets/build-tools/ets-loader/compile_plugin.js');
const rollup = require('<hvigor>/.../hvigor-arkts-base/node_modules/rollup');
const { commonPlugin } = require('<compose>/dist/src/plugins/common-plugin.js');
const nodeResolve = require('<compose>/dist/src/plugins/node-resolve/index.js').default;

const plugins = [
  commonPlugin(cfg),                 // 必须第一个
  nodeResolve({ extensions, modulePaths, sdkModulesPath, preferBuiltins: false }),
  ...p.sdkPlugins(cfg),
].filter((x) => x && typeof x === 'object');

const bundle = await rollup.rollup({
  input: { './EntryAbility': '<模块>/src/EntryAbility.ets' },
  plugins,
  context: 'globalThis',
});
await bundle.write({ dir: outDir, format: 'cjs' });
```

## 3. `share.projectConfig` 是唯一配置通道

所有插件通过 rollup 上下文的 `this.share.projectConfig` 读配置。这个对象**只能**由
`commonPlugin(cfg)` 的 `options()` 钩子建立（源码：`this.share.projectConfig = e`）。

直接 `require` ets-loader 的 `main.js` 并改 `main.projectConfig`，对插件**无效**——
这是本轮实测确认的，不要再走这条弯路。

## 4. 配置字段清单（按报错顺序实测）

| 缺失字段 | 报错 | 说明 |
|---|---|---|
| `sdkPath` | `Cannot read properties of undefined (reading 'toLowerCase')` | `commonPlugin` 里 `sdkPath.toLowerCase()` |
| `deviceTypes` | `Cannot read properties of undefined (reading 'deviceTypes')` | `etsChecker` 的 syscap 配置 |
| `cachePath` + `depName2RootPath` + `depName2DepInfo` | `o.forEach is not a function` | 后两者**必须是 `Map`**，不是普通对象 |
| `ohExports` | `Cannot convert undefined or null to object` | `rootFileNamesCollect` 里 `[...entryObj, ...ohExports]` |
| `entryObj` | 同上 | 形如 `{ './EntryAbility': '<绝对路径>' }` |
| `globalModulePaths` / `resolveModulePaths` | `"@ohos.app.ability.UIAbility" ... could not be resolved` | 指向 `<SDK>/ets/api` |
| `resolveConflictMode` | 同上 forEach 分支 | 用 `'none'` |

`nodeResolve` 的 `modulePaths` 里不要放不存在的目录（如 `oh_modules`），否则
`ENOENT: scandir`。

## 5. 编译器诊断怎么取

`rollup` 抛出的 `ArkTS Compiler Error` 只有一句话，细节在日志服务里：

```js
const BL = require('<hvigor>/.../hvigor-arkts-base/dist/index.js');
const events = BL.ArktsBaseLogger.getLogEvents();   // 每条 {time, category, level, msg, type}
```

注意包内路径：`hvigor-arkts-base/dist/index.js`，不要再套一层
`hvigor-arkts-base/node_modules/@ohos/hvigor-arkts-base`。

## 6. 当前卡点（未解决）

```
10505001 ArkTS Compiler Error
Declaration or statement expected. At File: <SDK>/ets/arkts/@arkts.lang.d.ets:71:1
Declaration expected.            At File: <SDK>/ets/arkts/@arkts.lang.d.ets:71:18
Unexpected keyword or identifier. At File: <SDK>/ets/arkts/@arkts.lang.d.ets:71:19
Cannot find name 'interface'.    At File: <SDK>/ets/arkts/@arkts.lang.d.ets:71:9
arkts-no-classes-as-obj          At File: <SDK>/ets/arkts/@arkts.lang.d.ets:79:13
```

TS 编译器把 `.d.ets` 当普通 `.ts` 解析。`<SDK>/ets/build-tools/ets-loader/lib/ets_checker.js`
导出的 `compilerOptions` 里确实有 `ets: true`，但 `createProgramPlugin` 建 Program 时没有采用它。

待查方向：
- `createProgramPlugin`（`lib/fast_build/ets_ui/rollup-plugin-ets-typescript.js` 的
  `ProgramMoment`）建 Program 的选项源。
- `hvigor` 调用时传入的 `otherPaths`（它被 `hijackTscCompilerOptionsPaths` 当作 `paths` 初值，
  本流程里是 undefined）。

## 7. 已排除的假设

- 不是 `runArkPack` 传参问题：直接把 config 喂给 `runArkPack` 时，配置不会转发给 ets-loader。
- 不是缺 `app.ets`：那是 FA 模型入口，Stage 不应需要；一旦补齐它，流程会退回 FA 的
  `buildManifest` 分支去读顶层 `manifest.pages`，而 Stage 的 pages 在 `module.pages`。
- 不是 `es2abc` 能单独用：`es2abc.exe` 直接编译 `.ets` 会报
  `SyntaxError: Decorators are not valid here`，装饰器必须先由 `etsTransform` 转换。

## 8. 2026-11-04 复测：能否只借 process_ui_syntax 而不跑 hvigor 链

目标：只要 AST 变换这一层，绕开 rollup 与 `projectConfig`。

已确认可行部分：

- `lib/process_ui_syntax.js`、`lib/process_struct_componentV2.js`、`lib/process_component_class.js`
  都能在 node 里直接 `require`，依赖只有 SDK 自带的 `typescript`（4.9.5，打了 struct 补丁）。
- `processUISyntax(m, isEntry, perfEvent, fileName, share, metaInfo)` 返回 tsc transformer 的双层工厂
  `context => sourceFile => sourceFile`，`m` 可以传 `null`。
- `process_component_class.processComponentClass(structNode, …)` 返回 `ClassDeclaration`；
  V2 走 `process_struct_componentV2.processStructComponentV2(structNode, errors, ctx, {hasReusableV2}, {hasReusePool})`。
- `getOrCreateStructInfo(name)` 返回完整结构表（`isComponentV2`、`paramDecoratorMap`、`localDecoratorSet` 等）。

仍然缺的上下文（按报错顺序）：

1. `ts.transform` + `compilerOptions`（含 `ets: true`）能跑，但输出的 struct 未被替换成 class。
2. `createProgram` + `emit` 崩在 `checkStructName`（需要全局结构名表）。
3. 直接调 `processStructComponentV2` 崩在 `reuseComponentsV2`，该字段来自
   `utils.storedFileInfo.getCurrentArkTsFile()`；该类（`SpecialArkTSFileInfo`）未导出，需自造对象再 `setCurrentArkTsFile`。
4. 补上文件上下文后崩在 `factory.updateDecorator`（`visitEachChildOfDecorator`），
   属补丁版 ts 内部 factory 变体，非 loader 层可补。

结论：这一层的入口都在，但运行需要一套补丁版 ts 的 factory/集合上下文；
只借函数不借上下文走不通，与前一轮记录一致。
