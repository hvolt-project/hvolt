@Entry
@Component
struct ChildParams {
  @State count: number = 3;
  @State label: string = 'n';

  build() {
    Column() {
      PropChild({ value: this.count, text: this.label })
      LinkChild({ total: $count })
    }
  }
}

@Component
struct PropChild {
  @Prop value: number = 0;
  @Prop text: string = '';

  build() {
    Text(`${this.value}${this.text}`)
  }
}

@Component
struct LinkChild {
  @Link total: number;

  build() {
    Text(`${this.total}`)
  }
}
