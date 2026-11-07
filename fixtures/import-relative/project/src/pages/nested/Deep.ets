import { helper } from '../Helper';

@Entry
@Component
struct DeepPage {
  build() {
    Column() {
      Text(helper())
    }
  }
}

export function deep(): string {
  return helper() + 'd';
}
