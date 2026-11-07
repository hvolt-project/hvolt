@AnimatableExtend(Text)
function animWidth(width: number) {
  .width(width)
}

@Entry
@Component
struct AnimatableExtendPage {
  @State target: number = 80;

  build() {
    Column() {
      Text('x').animWidth(this.target)
    }
  }
}
