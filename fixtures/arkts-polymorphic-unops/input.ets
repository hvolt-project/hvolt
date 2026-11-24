const text = "42";
const asNumber = +text;
const negated = -text;

@Entry
@Component
struct UnopsProbe {
  build() {
    Column() {
      Text(`${asNumber}${negated}`)
    }
  }
}
