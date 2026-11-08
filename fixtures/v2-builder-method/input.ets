@Entry
@ComponentV2
struct V2Builder {
  @Local label: string = 'l';

  @Builder
  chip(t: string) {
    ForEach([t], (item: string) => {
      Text(item)
    })
  }

  build() {
    Column() {
      this.chip(this.label)
    }
  }

  tail(): number {
    return 1;
  }
}
