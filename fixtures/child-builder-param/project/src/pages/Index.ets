@Component
struct Child {
  @BuilderParam content: () => void

  build() {
    Column() {
      this.content()
    }
  }
}

@Entry
@Component
struct Index {
  build() {
    Column() {
      Child() {
        ForEach(['inside'], (item: string) => {
          Text(item)
            .fontSize(16)
        })
      }
    }
  }
}
