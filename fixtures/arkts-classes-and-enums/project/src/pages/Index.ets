enum Mixed {
  A = 1,
  B = "b",
}

class Holder {
  secret: number = 1;
}

type Factory = new () => Holder;

function useClassValue() {
  return Holder;
}

@Entry
@Component
struct ArktsClassesAndEnums {
  build() {
    Column() {
      Text(`${useClassValue()}`)
    }
  }
}
