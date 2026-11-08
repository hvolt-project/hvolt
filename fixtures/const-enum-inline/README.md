# const-enum-inline

`const enum` 的成员访问在参考实现里被内联成常量，普通 `enum` **不**内联；两种 `enum` 的
**声明**都按常量值重写。`lower/enum_init.rs` 的 `fill_enum_initializers` 按这个 fixture 实现。

## 参考的发射（golden/Index.ts）

| 源码 | golden 发射 | 行 |
| --- | --- | --- |
| `ConstKind.A`（`A = 1`），用于属性 | `Text.fontSize(1)` | L83 |
| `ConstKind.B`（`B = A * 2`），用于属性 | `Text.layoutWeight(2)` | L84 |
| `ConstKind.B`，用于字段初始化 `private base = ConstKind.B` | `this.base = 2;` | L47 |
| `ConstStr.X`（`X = 'x'`） | `Text.width("x")` | L89 |
| `ConstMix.A` / `ConstMix.C`（隐式 `0` / `6`） | `Text.fontSize(0)` / `Text.layoutWeight(6)` | L94-95 |
| `ConstHex.H`（`H = 0x10`） | `Text.layoutWeight(16)` | L111 |
| `ConstNeg.N`（`N = -1`） | `Text.layoutWeight(-1)` | L116 |
| `PlainKind.P` / `.Q`（普通 `enum`） | **原样** `PlainKind.P` / `PlainKind.Q` | L100-101 |
| `PlainExpr.B`（普通 `enum`） | **原样** `PlainExpr.B` | L106 |
| 声明 `const enum ConstKind { A = 1, B = A * 2 }` | `A = 1, B = 2`（表达式折算） | L10-13 |
| 声明 `enum PlainExpr { A = 1, B = A * 2 }` | `A = 1, B = 2`（普通 enum 也折算） | L28-30 |
| 声明 `const enum ConstStr { X = 'x' }` | `X = "x"`（引号归一，AST 等价） | L14-17 |
| 声明 `const enum ConstHex { H = 0x10 }` | `H = 16`（数值归一成十进制） | L32-34 |

| `let typedValue: ConstMix.A = ConstMix.A`（声明注解里的成员类型） | `let typedValue: ConstMix.A = 0;` —— **类型不替换，值替换** | L39 |
| `function takesMember(a: ConstMix.A)`（参数表） | `a: ConstMix.A` 原样 | L41 |
| `function returnsMember(): ConstMix.C`（返回类型） | `: ConstMix.C` 原样，`return 6;` | L44-45 |
| `type MemberUnion = ConstMix.A \| ConstMix.C`（联合类型） | 原样 | L47 |

要点：

1. **内联不限属性位**：字段初始化器里的 `ConstKind.B` 同样变成 `2`（L34）。
2. **声明也要重写**：参考不是「保持源码写法」，而是把常量值重新发射出来 —— `B = A * 2` → `B = 2`、
   `0x10` → `16`。这两条都是 AST 级差异（`compare.js` 比的是节点形状与字面量文本），不是排版差异。
3. **普通 `enum` 只重写声明，不内联访问**：`PlainExpr.B` 保持成员访问（L110），但它的声明里
   `B = A * 2` 变成 `B = 2`（L29）。
4. 字符串成员保留源码写法即可（`'x'` 与 `"x"` 同 AST），所以实现不归一引号。
5. **类型位置一律不内联**（声明注解 L39 / 参数表 L41 / 返回类型 L44 / 联合类型 L47），只有值被替换。
   判据不是 token 形状而是 AST：类型位置的 `K.A` 在 TypeScript 里是 qualified type name，
   永远不会解析成成员表达式，所以按 `MemberExpression` 收集值位就够了。
   这条不是随便能写的：成员类型要求该枚举所有成员的初始化器都是字面量，否则 ArkTS 直接报
   `10505001 Enum type 'ConstKind' has members with initializers that are not literals`
   —— 所以 `ConstKind`（`B = A * 2`）不能当类型用，本 fixture 用的是 `ConstMix`。

## 实现

`src/build/lower/enum_init.rs` 的 `fill_enum_initializers`（`ets_transpile.rs:69` /
`ets_plain.rs:112` 的最后一个 pass，两条路径都经过它）：

* 每个成员算一个常量值：字面量、引用本枚举里已算出的成员、`+ - * / % ** << >> >>> & | ^ ~`、括号；
* 能算出来的显式初始化器按常量值重写（源码已经是同一个字面量时不写，避免无谓改动）；
* 隐式成员补 `= 值`；
* `const enum` 且每个成员都有常量值时，把 `Name.Member` 的每一处替换成字面量；
  普通 `enum` 只走上一条，不建表；
* 值位判定走 oxc AST：只替换 `MemberExpression`（`K.A`），类型位置是 `TSQualifiedName`、
  不会出现在收集里；文本解析失败时整份不内联（宁可不做）。
* 项目级表：`set_const_enums` 由 `ets_assemble` 在降级任何文件之前填好（跨文件的形态见
  `fixtures/const-enum-import`）。

## 复现

```sh
scripts/oracle/new-fixture.sh const-enum-inline
HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh const-enum-inline
scripts/oracle/all.sh const-enum-inline     # PASS
```

跨文件的那一形态在 `fixtures/const-enum-import`（仍是已知缺口）。