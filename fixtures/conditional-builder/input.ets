@Builder
function MyMenu() {
  Menu() {
    MenuItem({ content: 'mine' })
  }
}

@Builder
function OtherMenu() {
  Menu() {
    MenuItem({ content: 'other' })
  }
}

@Builder
function ArgMenu(label: string) {
  Menu() {
    MenuItem({ content: label })
  }
}

@Builder
function UnitOne() {
  Text('one')
}

@Entry
@Component
struct Index {
  @State cond: boolean = false

  @Builder
  ThisMenu() {
    Menu() {
      MenuItem({ content: 'this' })
    }
  }

  @Builder
  ThisArg(label: string) {
    Menu() {
      MenuItem({ content: label })
    }
  }

  @Builder
  UnitTwo() {
    Text('two')
  }

  @Builder
  emptyActions() {
    Text('none')
  }

  @BuilderParam actions: () => void = this.emptyActions

  build() {
    Column() {
      Text('menu')
        .bindMenu(this.cond ? MyMenu() : null)

      Text('context')
        .bindContextMenu(this.cond ? MyMenu() : OtherMenu(), ResponseType.RightClick)

      Text('this-menu')
        .bindMenu(this.cond ? this.ThisMenu() : null)

      Text('cover')
        .bindContentCover(this.cond, this.cond ? this.ThisMenu() : null)

      Text('reversed')
        .bindMenu(this.cond ? null : MyMenu())

      Text('arg-global')
        .bindMenu(this.cond ? ArgMenu('y') : null)

      Text('arg-this')
        .bindMenu(this.cond ? this.ThisArg('x') : null)

      Text('nested')
        .bindMenu(this.cond ? MyMenu() : this.cond ? OtherMenu() : null)

      Text('ident-this')
        .bindMenu(this.cond ? this.actions : null)

      Text('ident-global')
        .bindMenu(this.cond ? MyMenu : null)

      Text('second-arg')
        .bindMenu(this.cond, this.cond ? MyMenu() : null)

      TextInput({ placeholder: 'unit-global' })
        .showUnit(this.cond ? UnitOne() : null)

      TextInput({ placeholder: 'unit-this' })
        .showUnit(this.cond ? this.UnitTwo() : null)

      Text('popup-global')
        .bindPopup(this.cond, { builder: this.cond ? MyMenu() : null, onWillDismiss: false })

      Text('popup-this')
        .bindPopup(this.cond, { builder: this.cond ? this.ThisMenu() : null, onWillDismiss: false })

      Text('popup-param')
        .bindPopup(this.cond, { builder: this.actions, onWillDismiss: false })

      Text('popup-global-ident')
        .bindPopup(this.cond, { builder: MyMenu, onWillDismiss: false })
    }
  }
}