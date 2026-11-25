function pair(first: number, second: number): number {
  return first + second;
}

@Entry
@Component
struct ArityProbe {
  build() {
    Column() {
      Text(`${pair(1)}`)
    }
  }
}
