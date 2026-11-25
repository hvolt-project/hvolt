function identity<T extends number>(value: T): T {
  return value;
}

@Entry
@Component
struct GenericProbe {
  build() {
    Column() {
      Text(`${identity('text')}`)
    }
  }
}
