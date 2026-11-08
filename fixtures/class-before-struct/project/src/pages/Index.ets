import { StructFirst, ClassAfter } from '../shapes/struct-first'
import { OnlyClass } from '../shapes/class-only'
import { FuncThenStruct, leading } from '../shapes/func-first'

// PRO_INDEX
// PRO_INDEX_2
export class OptionsBefore {
  // CLS_BODY

  value: number = 1
}

@Entry
@Component
export struct BarAfter {
  // STRUCT_BODY
  @State n: number = 0

  build() {
    Column() {
      StructFirst()
      FuncThenStruct()
      Text(`${new OnlyClass().run() + leading() + new ClassAfter().v}`)
    }
  }
}
