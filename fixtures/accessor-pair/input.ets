// Counter-example: one name, two accessors. TypeScript allows a getter and a setter to share
// a name, so a duplicate-member rule must not fire here. This fixture exists to pin that.
@Entry
@Component
struct AccessorPair {
  private _value: string = '';

  get value(): string {
    return this._value;
  }

  set value(next: string) {
    this._value = next;
  }

  build() {
    Column() {
      Text(this.value)
    }
  }
}