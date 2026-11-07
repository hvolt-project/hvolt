@Observed
class Model {
  value: number = 0
}

@Entry
@Component
struct Index {
  @State count: number = 0
  @State flag: boolean = false
  @State model: Model = new Model()

  dialog: CustomDialogController = new CustomDialogController({
    builder: LinkDialog({ linked: this.count, copied: this.flag })
  })
  plainDialog: CustomDialogController = new CustomDialogController({
    builder: PlainChild({ linked: $count })
  })

  build() {
    Column() {
      LinkChild({ linked: $count, copied: this.flag })
      ReusableChild({ linked: this.count })
      ObjChild({ item: this.model })
      ParamChild({ value: 'p' })
    }
  }
}

@Component
struct LinkChild {
  @Link linked: number
  @Prop copied: boolean = false

  build() {
    Text(`${this.linked} ${this.copied}`)
  }
}

@Reusable
@Component
struct ReusableChild {
  @Link linked: number

  build() {
    Text(`${this.linked}`)
  }
}

@CustomDialog
struct LinkDialog {
  controller?: CustomDialogController
  @Link linked: number
  @Prop copied: boolean = false

  build() {
    Text(`${this.linked} ${this.copied}`)
  }
}

@Component
struct PlainChild {
  @Link linked: number

  build() {
    Text(`${this.linked}`)
  }
}

@Component
struct ObjChild {
  @ObjectLink item: Model

  build() {
    Text(`${this.item.value}`)
  }
}

@ComponentV2
struct ParamChild {
  @Param value: string = ''

  build() {
    Text(this.value)
  }
}