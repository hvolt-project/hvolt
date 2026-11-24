const obj = { a: 1 };
const key = "a";
const viaIndex = obj[key];

class Priv {
  private hidden: number = 1;
  readonly fixed: number = 2;
}

const instance = new Priv();
const hiddenValue = instance.hidden;

@Entry
@Component
struct DynamicPropProbe {
  build() {
    Column() {
      Text(`${viaIndex}${hiddenValue}`)
    }
  }
}
