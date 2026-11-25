import { first } from '../lib/a';

@Entry
@Component
struct CycleProbe {
  build() {
    Column() {
      Text(`${first()}`)
    }
  }
}
