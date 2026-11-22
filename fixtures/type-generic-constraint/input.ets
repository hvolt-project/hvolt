function constrained<T extends number>(value: T): T {
  return value;
}

function callConstrained() {
  return constrained("s");
}

function callConstrainedOk() {
  return constrained(1);
}

@Entry
@Component
struct TypeGenericConstraint {
  build() {
    Column() {
      Text("x")
    }
  }
}