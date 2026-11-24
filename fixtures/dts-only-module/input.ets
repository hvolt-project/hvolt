import { helper } from '../lib/types';

@Entry
@Component
struct DtsProbe {
  build() {
    Column() {
      Text(helper())
    }
  }
}
