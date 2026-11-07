// A `@CustomDialog` invoked with no parameters, from inside a `CustomDialogController`.
//
// The reference synthesises the construction argument list itself: `new ProbeDialog(this, {},
// undefined, -1, () => { }, { page: … })` — the second slot is the parameter object, and for a
// dialog with no parameters it is an empty `{}`. hvolt emitted that slot as nothing at all
// (`new ProbeDialog(this, , undefined, …)`), which is not valid TypeScript, so `es2abc` rejected
// the whole file. This fixture pins the empty object.
@CustomDialog
struct ProbeDialog {
  controller?: CustomDialogController

  build() {
    Column() {
      Text('probe')
    }
  }
}

@Entry
@Component
struct CustomDialogNoParams {
  private dlg: CustomDialogController = new CustomDialogController({
    builder: ProbeDialog(),
    autoCancel: true
  })

  build() {
    Column() {
      Button('open')
        .onClick(() => {
          this.dlg.open()
        })
    }
  }
}
