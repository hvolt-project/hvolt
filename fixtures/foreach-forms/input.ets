class RowItem {
  id: number = 0
  name: string = ''
}

const HEADS: string[] = ['a', 'b']

@Entry
@Component
struct ForEachForms {
  @State rows: RowItem[] = []

  build() {
    Column() {
      ForEach(HEADS, (head: string) => {
        Text(head)
      })
      ForEach(this.rows, (row: RowItem, index: number) => {
        Text(`${index}${row.name}`)
      }, (row: RowItem) => row.name)
      ForEach(this.rows, (row: RowItem) => {
        Text(row.name)
      }, (row: RowItem, index: number) => `${index}`)
      ForEach(this.rows, (row: RowItem, index: number) => {
        Text(`${index}${row.name}`)
      })
      ForEach(HEADS, (head: string) => {
        Row() {
          ForEach(this.rows, (inner: RowItem) => {
            Text(inner.name)
          })
        }
      }, (head: string) => head)
    }
  }
}
