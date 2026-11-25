
interface Box {
  value?: number;
}

function read(input: Box): number {
  return input?.value ?? 0;
}

@Entry
@Component
struct BatchProbe68 {
  build() {
    Column() {
      Text("x")
    }
  }
}
