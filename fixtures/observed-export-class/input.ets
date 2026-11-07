/**
 * Counts what the list holds.
 */
@Observed
export class ItemCount {
  total: number = 0
}

@Observed
class LocalItem {
  title: string = ''
}

@Entry
@Component
struct ObservedHost {
  @State count: ItemCount = new ItemCount()
  @State local: LocalItem = new LocalItem()

  build() {
    Column() {
      Text(`${this.count.total} ${this.local.title}`)
    }
  }
}
