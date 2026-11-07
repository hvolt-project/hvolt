import { Shape } from './Types';
import { describe } from './Helpers';

@Entry
@Component
struct ImportTypeAudit {
  private shape: Shape = { kind: 'circle' };
  @State label: string = describe(this.shape.kind);

  build() {
    Column() {
      Text(this.label)
    }
  }
}
