
interface Box {
  value: number;
}

function pick(input: Box): void {
  delete input.value;
}

@Entry
@Component
struct BatchProbe65 {
  build() {
    Column() {
      Text("x")
    }
  }
}
