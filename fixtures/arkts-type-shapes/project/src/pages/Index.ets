interface Indexed {
  [key: string]: number;
}

type Conditional<T> = T extends string ? number : boolean;

type Mapped = { [K in "a" | "b"]: number };

type Partly = Partial<Indexed>;

function typeQuery(value: number) {
  const same: typeof value = value;
  return same;
}

function objTypeLiteral() {
  const shape: { a: number } = { a: 1 };
  return shape.a;
}

@Entry
@Component
struct RulesProbe5 {
  build() {
    Column() {
      Text(`${typeQuery(1)}`)
    }
  }
}
