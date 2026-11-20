class Box<T> {
  item: T | null = null;

  take(): T | null {
    return this.item;
  }
}

function useBox(box: Box<number>) {
  return `${box.item}${box.take()}${box.notAReal}`;
}

@Entry
@Component
struct TypeGenericMember {
  build() {
    Column() {
      Text("x")
    }
  }
}
