class Holder {
  label: string = "x";

  size(): number {
    return 1;
  }
}

function useHolder(holder: Holder) {
  return `${holder.label}${holder.size()}${holder.notAReal}`;
}

@Entry
@Component
struct TypeClassMember {
  build() {
    Column() {
      Text("x")
    }
  }
}
