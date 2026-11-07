@AnimatableExtend(Text)
function animWidth(width: number) {
  .width(width)
}

@Reusable
@Component
struct Chip {
  @Prop label: string = '';

  build() {
    Text(this.label)
      .fontSize(12)
      .animWidth(20)
  }
}

@Entry
@Component
struct AnimatableReusablePage {
  build() {
    Column() {
      Chip({ label: 'a' })
    }
  }
}
