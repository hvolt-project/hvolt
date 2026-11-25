interface Shape {
  width: number;
}

function area(shape: Shape): number {
  return shape.width * shape.missing;
}

@Entry
@Component
struct MemberProbe {
  build() {
    Column() {
      Text(`${area({ width: 2 })}`)
    }
  }
}
