const parsed = JSON.parse("{}");
const keys = Object.keys(parsed);

@Entry
@Component
struct EsObjProbe {
  build() {
    Column() {
      Text(`${keys.length}`)
    }
  }
}
