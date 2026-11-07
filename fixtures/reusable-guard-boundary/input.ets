const GAP: number = 8

function getColor(value: number): ResourceColor {
  return Color.Red
}

class Shadow {
  static readonly GAP: number = 6
}

enum LocalKind {
  A = 1,
  B = 2
}

// (1) 普通成员（无装饰器） / (3) 全局常量
@Reusable
@Component
struct PlainMemberProbe {
  @State delay: number = 0
  private count: number = 3

  build() {
    Column() {
      Text('plain')
        .fontSize(this.count)
        .margin(GAP)
    }
  }
}

// (5) this 出现在函数调用实参里 / (6) 三元一侧读 this / 对照：纯函数调用
@Reusable
@Component
struct CallAndTernaryProbe {
  @State delay: number = 0
  @State big: boolean = false

  build() {
    Column() {
      Text('call')
        .fontColor(getColor(this.delay))
      Text('control')
        .fontColor(getColor(3))
      Text('ternary')
        .fontSize(this.big ? 20 : 12)
      Text('binary')
        .fontSize(12 + this.delay)
    }
  }
}

// (4a) @Styles 包裹组件：常量体（3 条属性，看发射顺序）/ 读状态体
@Reusable
@Component
struct StyleProbe {
  @State span: number = 10

  @Styles cardStyle() {
    .width(40)
    .height(20)
    .padding(5)
  }

  @Styles dynStyle() {
    .width(this.span)
  }

  @Styles pairStyle() {
    .height(30)
    .width(50)
  }

  build() {
    Column() {
      Text('styled')
        .cardStyle()
        .dynStyle()
      Text('styled-plain')
        .cardStyle()
      Text('styled-pair')
        .pairStyle()
    }
  }
}

// (4b) @BuilderParam 作为子内容
@Reusable
@Component
struct BuilderProbe {
  @BuilderParam footer: () => void

  @Builder
  guarded() {
    ForEach(['g'], (item: string) => {
      Text(item)
    })
  }

  build() {
    Column() {
      Text('host')
        .fontSize(12)
      this.footer()
      this.guarded()
    }
  }
}

// (2) build() 内的局部变量写不出来：编译器报
//   10905210 In an '@Entry' decorated component, the 'build' method can have only one root node
// 退而求其次测「模块级 let（运行期变量、无 this）」。
let runtimeWidth: number = 10

@Reusable
@Component
struct ModuleVarProbe {
  build() {
    Column() {
      Text('module-var')
        .width(runtimeWidth)
    }
  }
}

// 表达式形状判别：静态成员访问 / 对象字面量（套不套标识符）/ 一元 / 常量二元 / 模板串 / this 方法调用
@Reusable
@Component
struct FormProbe {
  private plain: number = 4

  widthOf(): number {
    return 10
  }

  build() {
    Column() {
      Text('static-member')
        .fontSize(Shadow.GAP)
      Text('object-literal')
        .margin({ top: 2 })
      Text('object-identifier')
        .margin({ top: GAP })
      Text('object-member')
        .margin({ top: this.plain })
      Text('unary')
        .layoutWeight(-1)
      Text('binary-const')
        .fontSize(2 * 3)
      Text('template')
        .width(`${this.plain}%`)
      Text('call-this')
        .width(this.widthOf())
    }
  }
}

// 枚举 / 类静态成员 / 本地声明的对比：区分「枚举特殊」与「本地声明 vs SDK 名」
@Reusable
@Component
struct EnumProbe {
  build() {
    Column() {
      Text('sdk-enum')
        .textAlign(TextAlign.End)
      Text('local-enum')
        .fontSize(LocalKind.A)
      Text('local-class-static')
        .fontSize(Shadow.GAP)
      Text('enum-mix')
        .fontSize(LocalKind.A)
        .textAlign(TextAlign.End)
        .width(Shadow.GAP)
    }
  }
}

// ===== A：枚举成员 / 类静态成员的机制 =====
const enum ConstKind {
  A = 1,
  B = 2
}

namespace NsHost {
  export const X: number = 1
}

class StaticFields {
  static readonly READONLY_GAP: number = 3
  static mut: number = 4
}

const kindVar: LocalKind = LocalKind.A
// 无类型注解：TS 推断成字面量类型 1（不是 enum member，也不是 number）
const AUTO = 1

@Reusable
@Component
struct EnumMechanismProbe {
  build() {
    Column() {
      Text('const-enum')
        .fontSize(ConstKind.A)
      Text('as-cast')
        .fontSize((LocalKind.A as number))
      Text('namespace')
        .fontSize(NsHost.X)
      Text('enum-var')
        .fontSize(kindVar)
      Text('const-auto')
        .fontSize(AUTO)
      Text('static-readonly')
        .fontSize(StaticFields.READONLY_GAP)
      Text('static-mutable')
        .fontSize(StaticFields.mut)
    }
  }
}

// ===== B：@Styles 与普通属性混排时的静态组顺序 =====
@Reusable
@Component
struct MixOrderProbe {
  @State dim: number = 12
  @State span: number = 10

  @Styles pairMixStyle() {
    .width(70)
    .height(80)
  }

  @Styles tripleMixStyle() {
    .padding(1)
    .margin(2)
    .backgroundColor(Color.Blue)
  }

  build() {
    Column() {
      Text('mix1')
        .width(40)
        .fontSize(this.dim)
        .pairMixStyle()
        .height(20)
        .margin(GAP)
        .padding(5)
      Text('mix2')
        .pairMixStyle()
        .tripleMixStyle()
        .width(90)
      Text('mix3')
        .fontColor(getColor(this.dim))
        .pairMixStyle()
        .fontColor(getColor(this.span))
    }
  }
}

// A：@Styles 的作用域——与 StyleProbe.cardStyle 重名但体不同（StyleProbe 是 40/20/5）
@Reusable
@Component
struct StylesScopeProbe {
  @Styles cardStyle() {
    .width(11)
    .height(12)
  }

  build() {
    Column() {
      Text('scope')
        .cardStyle()
    }
  }
}

// A3：顶层 @Styles function 与某 struct 的成员 @Styles 同名（谁赢），以及只有全局命中的调用
@Styles function globalStyle() {
  .width(99)
  .height(98)
}

@Reusable
@Component
struct GlobalStyleShadowProbe {
  @Styles globalStyle() {
    .width(21)
    .height(22)
  }

  build() {
    Column() {
      Text('global-shadowed')
        .globalStyle()
    }
  }
}

@Reusable
@Component
struct GlobalOnlyProbe {
  build() {
    Column() {
      Text('global-only')
        .globalStyle()
    }
  }
}

@Entry
@Component
struct Index {
  build() {
    Column() {
      PlainMemberProbe()
      CallAndTernaryProbe()
      StyleProbe()
      ModuleVarProbe()
      FormProbe()
      EnumMechanismProbe()
      MixOrderProbe()
      StylesScopeProbe()
      GlobalStyleShadowProbe()
      GlobalOnlyProbe()
      EnumProbe()
      BuilderProbe() {
        Text('inside')
          .fontSize(11)
      }
    }
  }
}