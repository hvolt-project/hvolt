class BasicDataSource implements IDataSource {
  private listeners: DataChangeListener[] = []
  private data: string[] = []

  totalCount(): number {
    return this.data.length
  }

  getData(index: number): string {
    return this.data[index]
  }

  registerDataChangeListener(listener: DataChangeListener): void {
    this.listeners.push(listener)
  }

  unregisterDataChangeListener(listener: DataChangeListener): void {
    const pos = this.listeners.indexOf(listener)
    if (pos >= 0) {
      this.listeners.splice(pos, 1)
    }
  }
}

@Entry
@Component
struct LazyForms {
  private source: BasicDataSource = new BasicDataSource()

  build() {
    List() {
      LazyForEach(this.source, (item: string) => {
        ListItem() {
          Text(item)
        }
      }, (item: string) => item)
      LazyForEach(this.source, (item: string, index: number) => {
        ListItem() {
          Text(`${index}${item}`)
        }
      })
      LazyForEach(this.source, (item: string) => {
        ListItem() {
          Text(item)
        }
      }, (item: string, index: number) => `${index}`)
    }
  }
}
