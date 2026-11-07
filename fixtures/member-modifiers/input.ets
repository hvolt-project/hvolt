@Entry
@ComponentV2
struct V2Private {
  private hidden: number = 3;
  public shown: number = 4;

  build() {
    Column() {
      V1Mods()
      Text(`${this.hidden}${this.shown}`)
    }
  }
}

@Component
struct V1Mods {
  public visible: number = 5;
  @Prop labeled: string = 'p';

  build() {
    Text(`${this.visible}${this.labeled}`)
  }
}
