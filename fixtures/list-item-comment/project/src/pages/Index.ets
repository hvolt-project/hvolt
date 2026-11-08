@Entry
@Component
struct ListItemComment {
  @State items: string[] = ['a', 'b']

  build() {
    Column() {
      List() {
        // SIMPLE_ITEM
        ListItem() {
          Text('one')
        }
        // AFTER_ITEM
        ListItem() {
          Text('two')
        }
      }
      .width('100%')

      List() {
        ForEach(this.items, (item: string) => {
          // FOREACH_ITEM
          ListItem() {
            Text(item)
          }
        }, (item: string) => item)
      }
      .height('40%')
    }
  }
}
