namespace Utils {
  export function size(): number {
    return 1;
  }
}

@Entry
@Component
struct NamespaceProbe {
  build() {
    Column() {
      Text(`${Utils.size()}`)
    }
  }
}
