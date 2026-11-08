// Enum members without an initializer take one from the previous member: the first is 0, and
// a member after an explicit one is that value + 1. A string member is never filled.
enum Mode {
  Off,
  On
}

enum Level {
  Low = 3,
  Mid,
  High
}

enum Mixed {
  A,
  B = 5,
  C
}

enum Named {
  X = 'x',
  Y = 'y'
}

@Entry
@Component
struct EnumInitializers {
  @State mode: Mode = Mode.On
  @State label: string = `${Level.High}/${Mixed.C}/${Named.Y}`

  build() {
    Column() {
      Text(`${this.mode} ${this.label}`)
    }
  }
}
