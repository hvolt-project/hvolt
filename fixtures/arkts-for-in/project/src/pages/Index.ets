function forIn(obj: Record<string, number>) {
  for (const key in obj) {
    return key;
  }
  return "";
}

function spread(numbers: number[]) {
  const copy = [...numbers];
  return copy.length;
}

@Entry
@Component
struct SpreadProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
