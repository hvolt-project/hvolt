
namespace Holder {
  const value = 1;
  if (value > 0) {
    const other = 2;
  }
}

@Entry
@Component
struct BatchProbe2 {
  build() {
    Column() {
      Text("x")
    }
  }
}
