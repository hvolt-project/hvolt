// `process_import.js::processImportNode` puts an imported `struct` into
// `componentCollection.customComponents`, and `bindComponentAttr` (process_component_build.js)
// reads that collection when it walks an attribute chain: a chain member that names a custom
// component is *not* an attribute, so the chain belongs to `__Common__` rather than to the
// component. The imported child below therefore takes the child-construction form with a
// `__Common__` frame around it, exactly as a same-file child does.
import { Card } from '../common/Card';

@Entry
@Component
struct ImportedChildChain {
  @State selected: boolean = false;

  build() {
    Column() {
      Card({ title: 'chained' })
        .transition(TransitionEffect.OPACITY)
        .width('100%')

      Card({ title: 'plain' })
    }
  }
}
