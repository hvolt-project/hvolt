function* generatorFn(): Generator<number> {
  yield 1;
}

const required = require("./helper");

@Entry
@Component
struct SyntaxProbe {
  build() {
    Column() {
      Text(`${required}`)
    }
  }
}
