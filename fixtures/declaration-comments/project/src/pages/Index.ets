// BUILDER_DECL_COMMENT
@Builder
export function declBuilder(label: string) {
  Text(label)
}

// FUNCTION_DECL_COMMENT
function plainHelper(value: number): number {
  return value + 1;
}

// INTERFACE_FIELD_COMMENT
interface DeclShape {
  // INTERFACE_MEMBER_COMMENT
  name: string
  size: number
}

// ENUM_MEMBER_COMMENT
enum DeclKind {
  // ENUM_FIRST
  First = 0,
  Second = 1
}

// STRUCT_DECL_COMMENT
@Component
export struct DeclHolder {
  // STATE_FIELD_COMMENT
  @State count: number = 0
  // PROP_FIELD_COMMENT
  @Prop title: string = ''
  // PLAIN_FIELD_COMMENT
  private label: string = 'x'

  build() {
    Column() {
      Text(this.title)
      Text(plainHelper(this.count).toString())
      this.item('a')
    }
  }

  // METHOD_DECL_COMMENT
  @Builder
  item(name: string) {
    Text(name)
  }
}

@Entry
@Component
struct DeclIndex {
  build() {
    Column() {
      declBuilder('d')
      DeclHolder({ title: 't' })
    }
  }
}
