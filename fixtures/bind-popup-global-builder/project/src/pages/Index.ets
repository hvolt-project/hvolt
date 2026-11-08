@Builder
function popupContent(label: string) {
  Text(label)
}

@Builder
function popupBuilder(ctx: object, tip: string, onClose: () => void) {
  Column() {
    Text(tip)
  }
}

@Builder
function selfArg(ctx: object) {
  Text('self')
}

@Builder
function twoArgs(first: string, second: string) {
  Text(first + second)
}

@Entry
@Component
struct BindPopupGlobalBuilder {
  @State shown: boolean = false;
  @State tips: string[] = ['a', 'b'];

  build() {
    Column() {
      Text('probe')
        .bindPopup(this.shown, {
          builder: popupContent('hello'),
          onWillDismiss: false
        })

      Text('arrow-arg')
        .bindPopup(this.shown, {
          builder: popupBuilder(this, this.tips[1], () => {
            Text('x')
          }),
          onWillDismiss: false
        })

      Text('self-arg')
        .bindPopup(this.shown, {
          builder: selfArg(this),
          onWillDismiss: false
        })

      Text('two-args')
        .bindPopup(this.shown, {
          builder: twoArgs('a', 'b'),
          onWillDismiss: false
        })
    }
  }
}