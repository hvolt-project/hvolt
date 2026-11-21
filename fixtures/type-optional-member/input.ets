interface OptionalHolder {
  count?: number;

  label: string;
}

function declOptional() {
  const a: OptionalHolder = { label: "x" };
  const b: OptionalHolder = {};
  return `${a.label}${b.label}`;
}

@Entry
@Component
struct TypeOptionalMember {
  build() {
    Column() {
      Text("x")
    }
  }
}
