import { helper } from '../lib/types';

const n: number = helper();

@Entry
@Component
struct DtsTypeProbe {
  build() {
    Column() {
      Text(`${n}`)
    }
  }
}
