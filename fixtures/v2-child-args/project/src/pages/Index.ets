@Entry
@ComponentV2
struct V2ArgsParent {
  @Local count: number = 1;

  build() {
    Column() {
      V2ArgsKid({ title: this.count + 1, onTap: () => {} })
    }
  }
}

@ComponentV2
struct V2ArgsKid {
  @Param title: number = 0;
  @Event onTap: () => void;

  build() {
    Text(`${this.title}`)
  }
}
