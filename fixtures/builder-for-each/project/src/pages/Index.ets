@Builder
export function ListBuilder(items: string[]) {
  ForEach(items, (item: string) => {
    Text(item)
  })
}

@Entry
@Component
struct BuilderForEach {
  build() {
    Column() {
      ListBuilder(['a', 'b'])
    }
  }
}
