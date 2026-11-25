function identity<T extends number>(value: T): T {
  return value;
}

const bad: number = identity('text');

@Entry
@Component
struct GenericProbe {
  build() {
    Column() {
      Text(`${bad}`)
    }
  }
}
