class Animal {
  name: string = "animal";
}

Animal.prototype.extra = 1;

const Anonymous = class {
  value: number = 1;
};

@Entry
@Component
struct ClassExprProbe {
  build() {
    Column() {
      Text(`${new Anonymous().value}${new Animal().name}`)
    }
  }
}
