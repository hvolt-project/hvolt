
class Box<T> {
  value: T | null = null;

  set(next: T): void {
    this.value = next;
  }
}

function read(): number {
  const box = new Box<number>();
  box.set(1);
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
