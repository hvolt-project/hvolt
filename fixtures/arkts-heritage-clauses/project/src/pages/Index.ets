interface AreaShape {
  area: number;
}

class BaseThing {
  size: number = 0;
}

class FromInterface extends AreaShape {
}

class ImplementsClass implements BaseThing {
}

class CtorReturn {
  constructor(): void {
  }
}

@Entry
@Component
struct HeritageProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
