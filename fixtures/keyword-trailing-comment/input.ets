@Entry
@Component
struct KeywordTrailing {
  @State x: boolean = true
  @State y: boolean = true

  build() {
    Column() {
      if (this.x) {
        Text('a')
      } else // ELSE_TRAILING
      {
        Text('b')
      }

      if (this.y) {
        Text('c')
      } // BRACE_BEFORE_ELSE
      else {
        Text('d')
      }

      if (this.x) {
        if (this.y) {
          Text('e')
        } // INNER_BRACE
      }
    }
  } // BUILD_END_TRAILING

  helper(): number {
    return 1 // STATEMENT_TRAILING
  }
}
