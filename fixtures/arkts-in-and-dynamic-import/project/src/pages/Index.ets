class Bag {
  size: number = 1;
}

function probeChain(bag: Bag | null) {
  return bag?.size;
}

function probeIn(bag: Bag) {
  return "size" in bag;
}

function probeInstance(bag: Bag) {
  return bag instanceof Bag;
}

function probeDyn() {
  return import('@ohos.app.ability.abilityManager');
}

@Entry
@Component
struct BatchProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
