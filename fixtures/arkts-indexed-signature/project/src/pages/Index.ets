type Mapper<T> = (value: T) => T;

interface Bag {
  [key: string]: number;
}

@Entry
@Component
struct AliasProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
