@Entry
@Component
struct VarDeclaration {
  build() {
    Column() {
      Text(`${probeValue}`)
    }
  }
}

var probeValue: string = "x";
