import Shape, { Area } from './Forms';
import type { Segment } from './Forms';

@Entry
@Component
struct ImportForms {
  @State summary: string = '';
  private seg: Segment = { from: 1 };

  aboutToAppear(): void {
    this.summary = Shape.label + String(Area) + Widget.tag + String(this.seg.from);
  }

  build() {
    Column() {
      Text(this.summary)
    }
  }
}

export { Area };
export { Area as AreaAlias } from './Forms';

export default class Widget {
  static readonly tag: string = 'widget';
}
