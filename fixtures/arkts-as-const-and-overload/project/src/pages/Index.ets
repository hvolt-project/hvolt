function overloaded(x: number): number;
function overloaded(x: string): string;
function overloaded(x: number | string): number | string {
  return x;
}

const constEnum = { A: 1 } as const;

@Entry
@Component
struct OverloadProbe {
  build() {
    Column() {
      Text(`${overloaded(1)}${overloaded("a")}${constEnum.A}`)
    }
  }
}
