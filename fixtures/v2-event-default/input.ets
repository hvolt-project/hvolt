@Entry
@ComponentV2
struct V2EventDefault {
  @Event onChange: (value: string) => void;

  build() {
    Column() {
      Text('event')
    }
  }
}
