# SPEC：`@Reusable` 的 `if (isInitialRender)` 拆分

给实现者的规格。写的是**要判什么、按什么顺序发射**，不是 Rust 代码。
所有「证据」都是 `fixtures/*/golden/` 的行号；`pn` = `fixtures/proxy-node-item/golden/Index.ts`，
`rgb` = `fixtures/reusable-guard-boundary/golden/Index.ts`。

---

## 0. 适用位置与前置条件

* 只在 **`@Reusable` 组件**的 `initialRender` 里有 `if (isInitialRender)`；非 `@Reusable` 组件没有
  guard，属性写按源码序全部发射（证据：`fixtures/attr-chain/golden/Index.ts` 的 `initialRender`
  里没有任何 `if (isInitialRender)`）。
* 入口：`src/build/stage/process_component_build.rs` 的 `emit_attribute_list`（当前
  `let guarded = attributes.iter().any(...)` 与「`if (isInitialRender) {` 包住全部属性」那段）。
* 非目标（本轮不改，改之前不要顺手改）：`@Styles` 名字作用域（跨组件串用）、`@Styles` 展开项的
  顺序（本规格只规定它在静态表里的位置，见 §4，实际行为由另外的修复对齐）、`const enum` 内联、
  `@Extend` 调用的注释复制。

---

## 1. 判定函数

```
attribute_is_static(args_text: &str, ctx: &ClassifyCtx) -> bool
```

* **输入**：`UiAttribute.args` 的**源码文本**（`src/build/stage/process_component_build.rs:195`，
  字段 `args` 就是「as written」的实参原文，可能为空串）。不需要完整表达式 AST——用
  `crate::build::lex::tokenize` 分词后按下面的形状判定即可。
* **输出**：`true` = 进 `if (isInitialRender)`（静态）；`false` = 在 guard 外每次重渲染都写（动态）。
* 一只属性写调一次；同一个组件帧里逐条独立判定（证据：`rgb` 同帧 `enum-mix`，`pn` 同帧
  `Text(delayText)` 里 `fontColor` 外、`fontSize/maxLines` 内）。

```
trait ClassifyCtx {
    /// 该名字在**本文件**里的声明种类；外部/SDK 名返回 None。
    fn declared_kind(name: &str) -> Option<DeclKind>;   // Enum | Class | Namespace | Var | Function
    /// 该名字是不是本组件的**带装饰器**成员（@State/@Prop/@Link/@Consume/@StorageLink/@BuilderParam/…）。
    fn is_decorated_member(name: &str) -> bool;
}
```

`DeclKind` 只需要区分「Enum」与「其它」；其余种类只用来回答「这个 `X` 是不是本地声明的非枚举」，
用于 §3.2 的降级。

---

## 2. 形状定义（token 级）

判定的第一层是**顶层形状**，按下面的顺序尝试匹配；匹配不到就落到 §3.9 的 Dynamic。

```
Literal             一个 Number/String/true/false/null 字面量
Template            反引号模板串（有无 ${} 都一样）
ThisMember          this . Ident
EnumLikeAccess      Ident . Ident
Arrow               (...) => { ... }   或   Ident => ...
ObjectLiteral       { key: value, ... }
As                  任意表达式 as 类型           （包括带括号的 `(X.Y as number)`）
Unary               -x / +x / !x / ~x
Binary              任意顶层二元运算符
Conditional         顶层 `? :`
Call                任意 `Ident[...]` / `X.Y(...)` / `this.m(...)`
Identifier          单个裸标识符
Paren               剥掉最外层成对括号后回到本表重新判定
```

括号是**剥掉**的：TS 的 AST 没有括号节点（`ParenthesizedExpression` 在 TS 里不存在，括号只是
`parenStart` 元数据），参考实现的判据看到的一定是括号内的形状。这条是推断，标在 §8。

---

## 3. 判定规则（逐条，附 golden 证据）

对 §2 得到的形状 `S`：

### 3.1 Literal → **静态**

| 表达式 | 归属 | 证据 |
| --- | --- | --- |
| `Text.maxLines(1)` | 内 | `pn:221` |
| `Text.fontSize(12)` | 内 | `pn:238` |
| `Text.fontSize(1)`（`const enum` 内联后） | 内 | `rgb:524` |
| `Text.width('100%')` 形态 | 内 | `pn:206`（`Flex.width('100%')`） |

### 3.2 `Ident . Ident`（EnumLikeAccess）→ 按**左侧名字的声明种类**

```
match ctx.declared_kind(X):
    Some(Enum)      -> 静态          # 本地枚举成员
    Some(Class)     -> 动态          # 类静态字段，readonly 与否一样
    Some(Namespace) -> 动态          # namespace 导出
    Some(Var)       -> 动态          # 模块变量/常量
    None            -> 静态          # 外部/SDK 名：按常量处理（降级策略，风险见 §8）
```

| 表达式 | 归属 | 证据 |
| --- | --- | --- |
| `LocalKind.A`（本地 `enum`） | 内 | `rgb:457` |
| `TextAlign.End`（SDK 枚举，文件内无声明 → None） | 内 | `pn:240` / `rgb:450` |
| `Shadow.GAP`（本地 `class` 静态） | 外 | `rgb:463` |
| `StaticFields.READONLY_GAP`（`static readonly`） | 外 | `rgb:550` |
| `StaticFields.mut`（可变静态） | 外 | `rgb:555` |
| `NsHost.X`（本地 `namespace` 导出） | 外 | `rgb:535` |

**不要**用「是不是 `readonly`」「是不是 `const`」判——`static readonly` 也是外（`rgb:550`），
`const GAP` 也是外（见 3.7）。

### 3.3 `this . member` → 看该成员带不带装饰器

```
ctx.is_decorated_member(member) ? 动态 : 静态
```

| 表达式 | 归属 | 证据 |
| --- | --- | --- |
| `this.count`（`private count = 3`） | **内** | `rgb:94` |
| `this.plain`（`private plain = 4`） | 内 | `rgb:392`（对象字面量里） |
| `this.delay` / `this.item.name` / `this.selected` / `this.icon_emphasize` / `this.isPXSecScreen` / `this.uiConfig.stringDisplayIndex` | 外 | `pn:236/248/250/339/347/357` |
| `this.span`（`@State`，经 `@Styles` 展开） | 外 | `rgb:224` |

`this.m(...)` 是 Call（§3.8），别在这里吞掉：`this.widthOf()` 在外（`rgb:413`）。

### 3.4 箭头函数 → **静态**

回调体**整棵跳过**，里面的 `this.` 不参与判定。

| 表达式 | 归属 | 证据 |
| --- | --- | --- |
| `Radio.onChange((isChecked) => { hilog.debug(… this.item.name …) })` | 内 | `pn:377` |

### 3.5 对象字面量 → **每个属性值都静态才静态**（递归）

| 表达式 | 归属 | 证据 |
| --- | --- | --- |
| `{ top: 2 }` | 内 | `rgb:380` |
| `{ left: 14, right: 16 }` | 内 | `pn:207` |
| `{ overflow: TextOverflow.Clip }` | 内 | `pn:351` |
| `{ top: GAP }`（标识符） | **外** | `rgb:386` |
| `{ top: this.plain }`（未装饰成员） | 内 | `rgb:392` |
| `{ checkedBackgroundColor: this.icon_emphasize }`（装饰成员） | 外 | `pn:250` |
| `{ maxWidth: this.uiConfig… ? '55%' : Infinity }`（条件表达式） | 外 | `pn:348` |

### 3.6 模板串 → **动态**（即使插值的是未装饰成员）

| 表达式 | 归属 | 证据 |
| --- | --- | --- |
| `` `${this.plain}%` `` | 外 | `rgb:408` |

### 3.7 裸标识符 → **动态**（模块 `const` / `let` / 字面量类型常量一律如此）

| 表达式 | 归属 | 证据 |
| --- | --- | --- |
| `GAP`（`const GAP: number = 8`） | 外 | `rgb:92` |
| `runtimeWidth`（模块 `let`） | 外 | `rgb:330` |
| `AUTO`（`const AUTO = 1`，字面量类型 `1`） | 外 | `rgb:545` |
| `kindVar`（`const kindVar: LocalKind = …`） | 外 | `rgb:540` |

即：**不做常量折叠，也不看类型是不是字面量类型**。`const AUTO = 1` 的类型就是 `1`，仍然是外。

### 3.8 调用 / 一元 / 二元 / 条件 / `as` → **动态**

| 表达式 | 归属 | 证据 |
| --- | --- | --- |
| `getColor(this.delay)` | 外 | `rgb:159` |
| `getColor(3)`（无 `this` 的纯调用） | 外 | `rgb:164` |
| `this.widthOf()` | 外 | `rgb:413` |
| `-1` | 外 | `rgb:398` |
| `2 * 3`（全常量二元） | 外 | `rgb:403` |
| `12 + this.delay` | 外 | `rgb:174` |
| `this.item.name === this.selected` | 外 | `pn:248` |
| `this.big ? 20 : 12`（整条，**不按分支拆**） | 外 | `rgb:169` |
| `(LocalKind.A as number)` | 外 | `rgb:530` |

### 3.9 其余形状 → **动态**（默认保守）

未列入 §3.1–§3.5 的一律动态。**不要**加「看起来像常量就进 guard」的补丁：`2 * 3`、`-1`
都反直觉地在外。

---

## 4. 发射顺序

```
emit_frame(attributes, comment, comp_name):
    dynamic = []                       # 保序
    static  = []                       # 保序，但 @Styles 项 unshift

    for attr in attributes:                        # 源码序
        if attr is @Extend(comp) call:             # 不放进去
            emit_extend(attr); continue            # 位置：guard 之前（见 4.1）
        if attr is @AnimatableExtend:              # 不放进去
            emit_animatable(attr, isInitialRender); continue
        if attr is animation | gesture:             # 现有特例，保持
            handle_special(attr); continue
        if attr is @Styles call:
            for (name, args) in style.body:        # body 源码序
                if attribute_is_static(args): static.unshift((name, args))
                else:                        dynamic.push((name, args))
            continue
        if attribute_is_static(attr.args): static.push(attr)
        else:                              dynamic.push(attr)

    emit dynamic                       # 源码序，放在 guard 之前
    if static is not empty:            # 静态集合为空 → 整帧不发 guard
        emit "if (isInitialRender) {" + static（表内顺序）+ "}"
```

四条硬约束，各自有证据：

1. **动态组在 guard 之外、在 guard 之前**，组内按源码序（`rgb:619-620` 的 `mix1`：`fontSize(this.dim)`
   然后 `margin(GAP)`；`rgb:644-645` 的 `mix3` 两条 `fontColor`）。
2. **静态集合为空 → 不发 guard**。证据：`rgb` 里 `static-member`(L365-366)、`unary`(L389-390)、
   `binary-const`(L394-395)、`template`(L399-400)、`call-this`(L404-405) 这些帧根本没有
   `if (isInitialRender)`。反过来，只要有一条静态属性就有 guard（`rgb:372` 的对象字面量帧）。
3. **`@Styles` 展开项 `unshift`，普通属性 `push`**（决策见 §5）。证据：`rgb:622-626` 的 `mix1`
   —— 源码是 `width(40) → fontSize(dyn) → pairMixStyle{width(70),height(80)} → height(20) →
   margin(dyn) → padding(5)`，发射成 `height(80); width(70); width(40); height(20); padding(5)`；
   `rgb:633-638` 的 `mix2` 两个 `@Styles` 连用，得到 `bg, margin(2), padding(1), height(80),
   width(70), width(90)`。
4. **`@Extend` / `@AnimatableExtend` 调用在 guard 之前**，不占静态集合（现有行为，别改）。
   证据：`pn:218-223`（`__Text__ItemTextStyle();` 在 `if (isInitialRender)` 之前）。

### 4.1 边界：不要顺手修的东西

* **注释复制**：`pn` 的 `// group 不要写死…` 参考发射 4 份（每条属性语句前一份），hvolt 只有 3 份且
  有一份落在帧外。这是独立缺口，本规格不覆盖。
* **`@Styles` 的名字作用域**：`rgb:684-687` 的 `scope` 帧证明参考按组件解析，hvolt 按名字串用。
  本规格只依赖「style.body 拿对了」；拿错了是另一个缺口。

---

## 5. 为什么静态组用 unshift（决策依据，写给审阅者）

参考实现的静态组不是源码序，也不是整体反序（`rgb:632-637`：`width(90)` 在最后而不是最前）。
唯一同时满足 `mix1` 与 `mix2` 的模型是：

> 遇到 `@Styles` 调用时，把它的 body **逐条按源码序 unshift** 到静态表头；普通静态属性 **push** 到尾。

`mix2` 推演：`[] → [w70] → [h80,w70] →（triple）[p1,h80,w70] → [m2,p1,h80,w70] →
[bg,m2,p1,h80,w70] →（plain）[…, w90]`，与 `rgb:632-637` 完全一致。
`mix1` 同理得到 `[h80, w70, w40, h20, p5]`，与 `rgb:622-626` 一致。

---

## 6. 需要的外部依赖（实现时要从哪里取）

| 需要的信息 | 现在有没有 | 取法 |
| --- | --- | --- |
| `this.<m>` 是否带装饰器 | **有**：`ComponentInfo.fields[].decorators`（`src/build/stage/utils.rs` 的 `FieldInfo`，`field.has("State")` 已在用） | 用 `fields` 建 `HashMap<name, decorators>` 传给 `ClassifyCtx` |
| `X` 是否本地 `enum` | **有一半**：`FileFacts.enums`（`src/build/stage/process_component_class.rs:49`，`BTreeSet<String>`，只收名字） | 直接复用；注意它现在只被 `is_simple_type_text` 用 |
| `X` 是否本地 class / namespace / 变量 | **没有** | 扩 `file_facts`：扫 `class` / `namespace` / `const|let|var` 后面的标识符，或从 lowering 已有的 oxc `Program` 取声明表。这一项是 §3.2 的 `Some(Class)/Some(Namespace)/Some(Var)` 分支所必需 |
| 外部名（SDK/跨文件） | 无 | 走 §3.2 的 `None → 静态` 降级，风险见 §8 |

**没有现成通用符号表**：`lower` 层拿到的是 oxc `Program`，但不保留声明表；`enum_init::fill_enum_initializers`
只算 enum 成员值；`ets_assemble::declares_runtime_enum` 只回「这个 enum 有没有字面量成员」。
所以 §3.2 的 `declared_kind` 是**新增**的一块，最省的做法是扩 `file_facts`（同一趟 token 扫描里
`class`/`namespace`/`const` 也能收）。

---

## 7. 验收口径

改完后逐 fixture 跑 `scripts/oracle/all.sh <name>`：

| fixture | 期望 |
| --- | --- |
| `proxy-node-item` | **AST 一致**（`AST differs` 消失）；`comment text differs` 仍在（注释复制是另一个缺口，见 §4.1）。判据：`pn` 里 15 条动态属性全在 guard 外，28 条静态全在内 |
| `reusable-guard-boundary` | 下面 5 处**应当**消除：所有 guard 归属；`styled`/`styled-plain` 的 `width(this.span)` 外、其余内。**仍会红**的 4 处（不属于本规格）：`styled`/`styled-plain`/`styled-pair`/`mix*` 的 `@Styles` 块顺序、`scope` 的名字作用域、`const-enum` 的 `1` vs `ConstKind.A` |
| `reusable` / `reusable-call-site` / `animatable-extend-reusable` / `link-call-site` / `v2-reusable*` | 不变（它们的属性全静态，guard 内容与顺序都不动） |

回归口径：本改动只影响 `@Reusable` 组件帧的**分组**，不影响非 `@Reusable` 组件、不影响
`@Extend`/`@AnimatableExtend`/`animation`/`gesture` 的现有位置。

---

## 8. 未决与风险（实现前要么补测、要么显式接受）

1. **外部名一律静态**（§3.2 的 `None` 分支）是降级策略。它让 `TextAlign.End`（SDK 枚举）对了，
   但会把「跨文件导入的 class 静态字段」也判成静态。没有反例读数，也没有正例证明参考这么做。
   真实工程里 `ProxyCardSize.Large` 走的就是这条（跨文件 enum），所以不能不去管。
2. **括号剥离**是推断（TS AST 无括号节点），未单独测 `(12)`。
3. **函数表达式**（`function(){…}` 作属性值）未测，暂按箭头同类处理。
4. **`@Styles` 展开出来的动态项**放进动态组的**位置**只有一条证据（`rgb:224`，`dynStyle` 是唯一的
   动态项），它与普通动态项的先后**未测**。规格暂按「展开序 push」。
5. **`@Styles` 展开项的静态顺序**在不同实现里也可能是「先收集再前置」，规格只钉住最终的发射序
   （§5 的 unshift 模型），不约束内部数据结构。
6. **`const enum` 内联与判据正交**：内联前 `ConstKind.A` 走 §3.2 的 `Enum` 分支仍是静态，内联后是
   §3.1 的字面量；**判据不需要为 `const enum` 单开分支**。内联是值层面的修复（§4.1 之外的另一条），
   放在 `lower/enum_init.rs` 同一处（`ets_transpile.rs:69` / `ets_plain.rs:112` 的最后一个 pass），
   理由：那是唯一已经在算 enum 成员常量值、且跑在参考实现看到的那份文本上的地方。