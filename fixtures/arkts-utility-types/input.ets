type RequiredShape = Required<{ a: number }>;
type Excluded = Exclude<string | number, string>;

@Entry
@Component
struct UtilityProbe {
  build() {
    Column() {
      Text(`${(undefined as unknown as RequiredShape) === null}`)
    }
  }
}
