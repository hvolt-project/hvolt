// PRO_SF
@Component
export struct StructFirst {
  // SF_BODY
  @State n: number = 0

  build() {
    Column() {
      Text('sf')
    }
  }
}

// BETWEEN_SF_CA
export class ClassAfter {
  // CA_BODY
  v: number = 2
}
