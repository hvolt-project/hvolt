
function tag(parts: string[], value: number): string {
  return parts[0];
}

function read(): string {
  return tag`x${1}`;
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
