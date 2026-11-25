
@Entry
@Component
struct OuterProbe {
  build() {
    Column() {
      Text('x')
    }
  }
}

function plain(): number {
  return 1;
}

@Entry
function decorated(): number {
  return 2;
}
