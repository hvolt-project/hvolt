
interface Factory {
  new (): object;
}

interface Caller {
  (value: number): string;
}

const inferred = [1, "a", true];

function pick(value: object): object {
  return value;
}

@Entry
@Component
struct BatchProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
