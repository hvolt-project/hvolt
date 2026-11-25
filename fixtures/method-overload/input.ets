// Counter-example: overload signatures share a name by design. If hvigor accepts this, a
// duplicate-member rule has to treat signature-only declarations (no body) as one member.
@Entry
@Component
struct Overloads {
  pick(value: number): number;

  pick(value: number): number {
    return value;
  }

  build() {
    Column() {
      Text(String(this.pick(1)))
    }
  }
}