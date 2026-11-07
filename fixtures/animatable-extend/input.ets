@AnimatableExtend(Text)
function animWidth(width: number) {
  .width(width)
}

@AnimatableExtend(Text)
function animClear() {
  .opacity(0)
}

@AnimatableExtend(Text)
function animBoth(w: number, h: number) {
  .width(w)
  .height(h)
}

@AnimatableExtend(Text)
function animEmpty() {
}

@Entry
@Component
struct AnimatableExtendPage {
  build() {
    Column() {
      Text('x')
    }
  }
}
