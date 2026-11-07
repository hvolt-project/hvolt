// A child that declares both a `@BuilderParam` and a `@Link` is the one call-site shape whose
// params object is rebuilt around the written fields: the `@Link` member still has to reach the
// child as the parent's backstore (`this.__count`), not as its accessor.
@Component
struct LinkItem {
  @BuilderParam content: () => void
  @Link linked: number
  @Prop copied: boolean = false

  build() {
    Column() {
      this.content()
      Text(`${this.linked} ${this.copied}`)
    }
  }
}

@Entry
@Component
struct Index {
  @State count: number = 0
  @State flag: boolean = false

  build() {
    Column() {
      LinkItem({ linked: this.count, copied: this.flag }) {
        ForEach(['body'], (item: string) => {
          Text(item)
        })
      }
    }
  }
}
