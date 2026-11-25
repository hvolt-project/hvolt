
function Configure(options: object): object {
  return options;
}

@Configure({ level: 1 })
class Configured {
  value: number = 0;
}

@Entry
@Component
struct BatchProbe7 {
  build() {
    Column() {
      Text("x")
    }
  }
}
