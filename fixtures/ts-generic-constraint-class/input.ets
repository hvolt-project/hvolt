
interface Named {
  name: string;
}

class Holder<T extends Named> {
  item: T | null = null;
}

function read(): number {
  return 1;
}

@Entry
@Component
struct BatchProbe87 {
  build() {
    Column() {
      Text("x")
    }
  }
}
