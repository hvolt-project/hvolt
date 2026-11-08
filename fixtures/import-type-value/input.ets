import { Mode, Shape } from './Model';

// `Shape` is an interface, so it has no runtime export: the import has to be `import type`
// even though a syntactic reading counts the `implements` clause as a value position. `Mode`
// is an enum and stays a value import — the two are split into their own statements.
class Circle implements Shape {
  kind: string = 'circle';
  mode: Mode = Mode.Fast;
}

@Entry
@Component
struct ImportTypeValue {
  @State shape: Shape = new Circle();

  build() {
    Column() {
      Text(`${this.shape.kind} ${Mode.Slow}`)
    }
  }
}
