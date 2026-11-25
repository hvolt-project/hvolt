
function markIt(target: object, key: string, desc: object): void {
}

@markIt
@Sendable
function shared(): number {
  return 1;
}

@Entry
@Component
struct BatchProbe45 {
  build() {
    Column() {
      Text("x")
    }
  }
}
