import { helper } from './Helper';
import { deep } from './nested/Deep';

@Entry
@Component
struct RelativeImports {
  @State text: string = '';

  aboutToAppear(): void {
    this.text = helper() + deep();
  }

  build() {
    Column() {
      Text(this.text)
    }
  }
}
