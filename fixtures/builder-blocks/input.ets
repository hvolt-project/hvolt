@Entry
@Component
struct CallView {
  @Builder label(text: string) {
    Text(text)
  }

  build() {
    Column({ space: 4 }) {
      Row() {
        Column() {
          Text('deep')
        }
      }
      Column() {}
      this.label('method')
    }
  }
}
