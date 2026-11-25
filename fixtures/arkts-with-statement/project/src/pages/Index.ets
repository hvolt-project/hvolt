function useWith(target: Record<string, number>): number {
  with (target) {
    return 1;
  }
}

@Entry
@Component
struct WithProbe {
  target: Record<string, number> = {};

  build() {
    Column() {
      Text(`${useWith(this.target)}`)
    }
  }
}
