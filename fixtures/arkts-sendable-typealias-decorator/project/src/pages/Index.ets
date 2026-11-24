
function markIt(target: object, key: string, desc: object): void {
}

@markIt
@Sendable
type SharedFn = () => number;

@Entry
@Component
struct BatchProbe45 {
  build() {
    Column() {
      Text("x")
    }
  }
}
