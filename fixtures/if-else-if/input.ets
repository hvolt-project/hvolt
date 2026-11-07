@Entry
@Component
struct BranchForms {
  @State step: number = 0

  build() {
    Column() {
      if (this.step === 0) {
        Text('zero')
      } else if (this.step === 1) {
        if (this.step > 10) {
          Text('big')
        } else {
          Text('small')
        }
      } else if (this.step === 2) {
        Row() {
          Text('two')
        }
      } else {
        Text('many')
      }
    }
  }
}
