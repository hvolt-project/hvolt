function isString(value: string | number): value is string {
  return typeof value === "string";
}

function tagged(): void {
}

tagged.extra = 1;

@Entry
@Component
struct IsAndPropsProbe {
  build() {
    Column() {
      Text(`${isString(1)}`)
    }
  }
}
