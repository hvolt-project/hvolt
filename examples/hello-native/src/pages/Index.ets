import { add } from 'libhello.so';

@Entry
@Component
struct Index {
  @State a: number = 3;
  @State b: number = 4;
  @State sum: number = 0;

  build() {
    Column({ space: 16 }) {
      Text('hello-native')
        .fontSize(28)
        .fontWeight(FontWeight.Bold)
      Text(`${this.a} + ${this.b} = ${this.sum}`)
        .fontSize(20)
      Button('call native add()')
        .onClick(() => {
          this.sum = add(this.a, this.b);
        })
    }
    .width('100%')
    .padding(24)
    .justifyContent(FlexAlign.Center)
  }
}
