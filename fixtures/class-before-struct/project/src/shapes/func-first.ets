// PRO_FF
export function leading(): number {
  return 1
}

// BETWEEN_FF
@Component
export struct FuncThenStruct {
  // FTS_BODY
  build() {
    Column() {
      Text('f')
    }
  }
}
