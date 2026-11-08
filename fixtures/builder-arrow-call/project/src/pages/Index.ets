// Two statement-level builder calls in the same component: one written at the top of `build()`,
// one written inside an inline arrow. The golden decides which of them takes the bound form.

@Entry
@Component
struct BuilderArrowCall {
  @State items: string[] = ['a']

  @Builder
  itemEnd(id: string) {
    Text(id).fontSize(12)
  }

  build() {
    Column() {
      // 1 a statement-level call at the top of build()
      this.itemEnd('top')
      List() {
        ForEach(this.items, (item: string) => {
          ListItem() {
            Text(item)
          }
          .swipeAction({
            end: {
              builder: () => {
                // 2 a statement-level call inside an inline arrow
                this.itemEnd(item)
              },
              actionAreaDistance: 56
            }
          })
        })
      }
    }
  }
}
