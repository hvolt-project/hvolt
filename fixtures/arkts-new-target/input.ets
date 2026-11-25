const pattern = /ab+c/gi;

function makeTarget(): unknown {
  return new.target;
}

@Entry
@Component
struct SyntaxBatchProbe {
  build() {
    Column() {
      Text(`${pattern.test("abc")}`)
    }
  }
}
