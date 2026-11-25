interface Callable {
  (value: number): number;
}

function symbolType(value: symbol): symbol {
  return value;
}

@Entry
@Component
struct ArktsCallSignatureAndSymbol {
  build() {
    Column() {
      Text("x")
    }
  }
}