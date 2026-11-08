@Entry
@Component
struct FormsPage {
  build() {
    Column() {
      Text('forms')
    }
  }
}

export interface Segment {
  from: number;
}

export const Area: number = 42;

export default class Shape {
  static label: string = 'shape';
}
