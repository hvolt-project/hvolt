@Entry
@Component
struct ButtonCreateMethod {
  private label: string = 'from field'

  build() {
    Column() {
      Button()
      Button({ type: ButtonType.Circle, stateEffect: false })
      Button('label-only')
      Button(this.label)
      Button($r('app.string.app_name'))
      Button({ type: ButtonType.Circle, stateEffect: true }) {
        Text('circle child')
      }
      Row() {
        Text('row child')
      }
      Row()
      Text('leaf')
    }
  }
}
