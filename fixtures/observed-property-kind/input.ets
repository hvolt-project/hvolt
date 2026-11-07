// 状态字段的 backstore 类型由字段类型决定：哪一类是 `ObservedPropertySimplePU`、
// 哪一类是 `ObservedPropertyObjectPU`；`@Prop`/`@Link` 那族同理（Simple/Object OneWay）。
// 本地类型别名：字符串字面量联合 / number
type LocalAlias = 'a' | 'b'
type NumberAlias = number
type StringPrimAlias = string
type BoolAlias = boolean
type EnumAlias = LocalKind
type TwoPrimAlias = string | number

interface SomeInterface {
  name: string
}

class AppConfig {
  name: string = ''
}

@Observed
class SomeClass {
  value: number = 0
  name: string = ''
}

enum LocalKind {
  A = 1,
  B = 2
}

@Component
struct PropHost {
  @Prop propObject: SomeClass = new SomeClass()
  @Prop propNumber: number = 0
  @Link linkObject: AppConfig
  @ObjectLink objectLink: SomeClass

  build() {
    Column() {
      Text(`${this.propNumber}`)
    }
  }
}

@Component
struct ConsumeHost {
  @Consume consumedConfig: AppConfig
  @Consume consumedNumber: number

  build() {
    Column() {
      Text(`${this.consumedNumber}`)
    }
  }
}

@Entry
@Component
struct ObservedPropertyKind {
  // 12. @Provide：对象与原始类型（backstore 种类应与 @State 同规）
  @Provide consumedConfig: AppConfig = new AppConfig()
  @Provide consumedNumber: number = 0
  // 1. 原始类型
  @State count: number = 0
  @State label: string = 'a'
  @State flag: boolean = false
  // 2. 对象字面量初值（本地 class）
  @State config: AppConfig = { name: 'x' }
  // 3. 类实例
  @State item: SomeClass = new SomeClass()
  // 4. 数组
  @State list: number[] = []
  @State objs: SomeClass[] = []
  // 5. Map
  @State m: Map<string, number> = new Map()
  // 6. 联合与可选
  @State maybe: string | undefined = undefined
  @State maybeBool: boolean | undefined = false
  @State opt?: number = undefined
  // 8. 枚举
  @State kind: LocalKind = LocalKind.A
  // 10. 本地类型别名
  @State localAlias: LocalAlias = 'a'
  @State numberAlias: NumberAlias = 1
  @State stringPrimAlias: StringPrimAlias = 's'
  @State boolAlias: BoolAlias = true
  @State enumAlias: EnumAlias = LocalKind.A
  @State twoPrimAlias: TwoPrimAlias = 's'
  @State inlineLiteralUnion: 'a' | 'b' = 'a'
  @State inlinePrimUnion: number | string = 1
  // 11. interface / Array 角度括号
  @State iface: SomeInterface = { name: 'n' }
  @State angleList: Array<number> = []

  build() {
    Column() {
      Text(`${this.count}${this.label}${this.flag}${this.kind}`)
      PropHost({ propObject: this.item, propNumber: 1, linkObject: $config, objectLink: this.item })
      ConsumeHost()
    }
  }
}