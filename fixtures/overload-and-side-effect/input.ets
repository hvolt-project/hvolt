import '../lib/side';

function overloaded(a: number): number;
function overloaded(a: string): string;
function overloaded(a: number | string): number | string {
  return a;
}

@Entry
@Component
struct OverloadProbe {
  build() {
    Column() {
      Text(`${overloaded(1)}`)
    }
  }
}
