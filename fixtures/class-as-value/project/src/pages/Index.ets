class PointX {
  x: number = 0;
}

const alias = PointX;
const name = alias.name;

@Entry
@Component
struct ClassAsObjProbe {
  build() {
    Column() {
      Text(name)
    }
  }
}
