
function read(): number {
  const ctx = getContext();
  return ctx.missingProp.length;
}

@Entry
@Component
struct BatchProbe60 {
  build() {
    Column() {
      Text("getContext()")
    }
  }
}
