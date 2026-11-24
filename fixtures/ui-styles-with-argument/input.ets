@Builder
function buildItem(label: string): void {
  Text(label)
}

@Styles
function styleWithParam(size: number) {
  .width(size)
}

@Entry
@Component
struct BuilderProbe {
  build() {
    Column() {
      buildItem("a")
    }
  }
}
