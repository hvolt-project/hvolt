import { ObservedCounter } from '../common/ObservedModel';

// The page is ordinary; the rule under test lives in the imported file, which declares no
// struct and so goes through the plain lowering (`extra-files.json` names it for comparison).
@Entry
@Component
struct PlainObservedPage {
  @State counter: ObservedCounter = new ObservedCounter();

  build() {
    Column() {
      Text(`${this.counter.total}`)
    }
  }
}
