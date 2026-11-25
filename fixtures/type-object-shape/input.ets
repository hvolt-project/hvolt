interface Holder {
  count: number;
}

function takeHolder(holder: Holder): number {
  return holder.count;
}

function objectShapes() {
  const h: Holder = { count: "s" };
  return takeHolder({ count: "x" }) + h.count;
}

@Entry
@Component
struct TypeObjectShape {
  build() {
    Column() {
      Text("x")
    }
  }
}