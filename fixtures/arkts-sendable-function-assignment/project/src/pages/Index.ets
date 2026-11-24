
@Sendable
type SharedFn = (value: number) => number;

const used: SharedFn = (value: number): number => value;

@Entry
@Component
struct BatchProbe41 {
  build() {
    Column() {
      Text("x")
    }
  }
}
