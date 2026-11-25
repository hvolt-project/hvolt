@Component
struct MissingBuild {
  message: string = "x";
}

@Entry
@Component
struct EntryProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
