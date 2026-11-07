@Component
export struct CardTitle {
  title: ResourceStr = ''

  build() {
    Row() {
      Text(this.title)
    }
  }
}
