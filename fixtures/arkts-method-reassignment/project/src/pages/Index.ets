class Holder {
  static count: number = 0;

  static {
    Holder.count = 1;
  }

  method(): number {
    return 1;
  }
}

function reassign(target: Holder): void {
  target.method = () => 2;
}

@Entry
@Component
struct StaticBlockProbe {
  build() {
    Column() {
      Text(`${Holder.count}`)
    }
  }
}
