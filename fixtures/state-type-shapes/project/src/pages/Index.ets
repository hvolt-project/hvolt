enum Kind { A, B }

@Entry
@Component
struct TypeProbe {
  @State both: string | number = 1;
  @State maybe: number | undefined = undefined;
  @State literals: 'a' | 'b' = 'a';
  @State single: 'a' = 'a';
  @State kind: Kind | undefined = undefined;

  build() {
    Column() {
      Text(`${this.both}${this.maybe}${this.literals}${this.single}${this.kind}`)
    }
  }
}
