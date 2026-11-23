@CustomDialog
struct MyDialog {
  controller?: CustomDialogController;

  build() {
    Column() {
      Text("dialog")
    }
  }
}

@Entry
@Component
struct Batch9Probe {
  dialog: CustomDialogController = new CustomDialogController({ nope: MyDialog() });

  build() {
    Column() {
      Button("open")
        .onClick(() => {
          this.dialog.open();
        })
    }
  }
}
