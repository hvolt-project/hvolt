@Component
struct ChildCard {
  @Prop label: string;

  build() {
    Column() {
      Text(this.label)
    }
  }
}

@Entry
@Component
struct UiComponentArgument {
  build() {
    Column() {
      ChildCard({ label: 1 })
      ChildCard({ label: "ok" })
    }
  }
}