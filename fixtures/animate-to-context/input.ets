// `processAnimateToOrImmediately` (process_ui_syntax.js) rewrites a bare `animateTo(…)` /
// `animateToImmediately(…)` call into `Context.animateTo(…)`.
@Entry
@Component
struct AnimatePage {
  @State span: number = 10;

  build() {
    Column() {
      Text('animate')
    }
  }

  aboutToAppear(): void {
    animateTo({ duration: 100 }, () => {
      this.span = 20;
    });
    animateToImmediately({ duration: 100 }, () => {
      this.span = 30;
    });
  }
}
