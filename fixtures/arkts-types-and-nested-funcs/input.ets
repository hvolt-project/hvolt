type Comparison = number & { compare: boolean };
type Conditional<T> = T extends string ? number : boolean;

function outer(): void {
  function inner(): void {
  }
  inner();
}

const sequence = (1, 2);

@Entry
@Component
struct Batch2Probe {
  build() {
    Column() {
      Text(`${sequence}`)
    }
  }
}
