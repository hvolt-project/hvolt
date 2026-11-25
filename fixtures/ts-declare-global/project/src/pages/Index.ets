
declare global {
  interface Window {
    extra: number;
  }
}

function read(input: Window): number {
  return input.extra;
}

@Entry
@Component
struct BatchProbe63 {
  build() {
    Column() {
      Text("x")
    }
  }
}
