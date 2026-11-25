class NodeHolder {
  ref: this | null = null;

  clone(): this {
    return this;
  }
}

const holder = new NodeHolder();
const flag = holder instanceof NodeHolder;
const withType: this = holder;

@Entry
@Component
struct ThisTypingProbe {
  build() {
    Column() {
      Text(`${flag}${withType !== null}`)
    }
  }
}
