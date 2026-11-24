const items: number[] = [1, 2, 3];
const index = 1;
const picked = items[index];
const literals = [4, 5];
const fromLiteral = literals[0];

@Entry
@Component
struct ArrayIndexProbe {
  build() {
    Column() {
      Text(`${picked}${fromLiteral}`)
    }
  }
}
