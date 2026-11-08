@Entry
@Component
struct HelperPage {
  build() {
    Column() {
      Text('helper')
    }
  }
}

export function helper(): string {
  return 'h';
}
