
function markIt(): ClassDecorator {
  return () => {};
}

@markIt()
@Sendable
class SharedDecorated {
  value: number = 0;
}

@Entry
@Component
struct BatchProbe41 {
  build() {
    Column() {
      Text("x")
    }
  }
}
