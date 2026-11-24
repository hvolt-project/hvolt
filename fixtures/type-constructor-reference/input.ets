class WithCtor {
  label: string;

  constructor(label: string) {
    this.label = label;
  }

  describe(): string {
    return this.label;
  }
}

const instance = new WithCtor("x");
const described = instance.describe();
const ctorRef = instance.constructor;

@Entry
@Component
struct CtorProbe {
  build() {
    Column() {
      Text(`${described}${ctorRef !== undefined}`)
    }
  }
}
