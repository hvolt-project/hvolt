@Entry
@Component
struct NonComponentStatement {
  private helper(): number {
    return 1;
  }

  @Builder
  buildable() {
    Text("built")
  }

  build() {
    Column() {
      this.helper();
      this.buildable();
    }
  }
}
