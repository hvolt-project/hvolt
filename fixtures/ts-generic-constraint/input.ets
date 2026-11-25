
interface Named {
  name: string;
}

function pick<T extends Named>(input: T): string {
  return input.name;
}

function callIt(): string {
  return pick(1);
}

@Entry
@Component
struct BatchProbe49 {
  build() {
    Column() {
      Text("x")
    }
  }
}
