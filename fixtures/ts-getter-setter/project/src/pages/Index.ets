
class Holder {
  private stored: number = 0;

  get value(): number {
    return this.stored;
  }

  set value(next: number) {
    this.stored = next;
  }
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
