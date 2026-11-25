interface Metrics {
  width: number;

  height: number;
}

type IndexedWidth = Metrics["width"];

function funcCalls(target: (value: number) => number): number {
  target.apply(null, [1]);
  target.call(null, 1);
  target.bind(null);
  return 0;
}

@Entry
@Component
struct ArktsIndexedAndApply {
  build() {
    Column() {
      Text("x")
    }
  }
}