// `const enum` 的成员访问在参考实现里被内联成常量；普通 `enum` 不内联。
// 本 fixture 钉住每一类形态的发射值。
const enum ConstKind {
  A = 1,
  B = A * 2
}

const enum ConstStr {
  X = 'x',
  Y = 'y'
}

const enum ConstMix {
  A,
  B = 5,
  C
}

enum PlainKind {
  P = 1,
  Q = 2
}

// 普通 enum 的**声明**也按常量值重写（B = A * 2 → B = 2），但成员访问不内联。
enum PlainExpr {
  A = 1,
  B = A * 2
}

const enum ConstHex {
  H = 0x10
}

const enum ConstNeg {
  N = -1
}

// 成员用作**类型**时，参考是否也把它换成字面量？（值是 `ConstKind.A`，类型也是）
let typedValue: ConstMix.A = ConstMix.A

// 成员作类型：参数表 / 返回类型 / 联合类型（ConstMix 的成员全是字面量，才允许这么写）
function takesMember(a: ConstMix.A): number {
  return a
}

function returnsMember(): ConstMix.C {
  return ConstMix.C
}

type MemberUnion = ConstMix.A | ConstMix.C

@Entry
@Component
struct ConstEnumInline {
  @State n: number = 0
  private base: number = ConstKind.B

  build() {
    Column() {
      Text('const-number')
        .fontSize(ConstKind.A)
        .layoutWeight(ConstKind.B)
      Text('const-string')
        .width(ConstStr.X)
      Text('const-mix')
        .fontSize(ConstMix.A)
        .layoutWeight(ConstMix.C)
      Text('plain-enum')
        .fontSize(PlainKind.P)
        .layoutWeight(PlainKind.Q)
      Text('plain-expr')
        .layoutWeight(PlainExpr.B)
      Text('const-hex')
        .layoutWeight(ConstHex.H)
      Text('const-neg')
        .layoutWeight(ConstNeg.N)
      Text('state')
        .fontSize(this.n)
    }
  }
}