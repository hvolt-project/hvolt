@Component
export struct Card {
  @Prop title: string = '';

  build() {
    Text(this.title)
  }
}
