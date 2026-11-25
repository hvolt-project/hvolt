// One name, two members: a state field and a method. The field wins at runtime, so the call
// site in build() is calling a string. This is the shape that took the app down on 2026-11-12
// (`sdkState is not callable`), reduced to the smallest case.
@Entry
@Component
struct MemberCollision {
  @State private sdkState: string = '';

  private sdkState(): string {
    return 'disk';
  }

  build() {
    Column() {
      Text(this.sdkState())
    }
  }
}