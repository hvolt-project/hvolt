function first<T>(items: T[]): T {
  return items[0];
}

const n: number = first<string>(["a"]);

@Entry
@Component
struct GenericProbe {
  build() {
    Column() {
      Text(`${n}`)
    }
  }
}
