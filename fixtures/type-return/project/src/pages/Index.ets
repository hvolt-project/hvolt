class Ticker {
  bump(step: number): number {
    return step;
  }
}

function returnsWrong(): number {
  return "s";
}

function methodWrong(ticker: Ticker) {
  ticker.bump("x");
}

@Entry
@Component
struct TypeReturn {
  build() {
    Column() {
      Text("x")
    }
  }
}
