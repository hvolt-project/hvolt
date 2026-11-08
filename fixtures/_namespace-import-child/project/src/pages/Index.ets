// `process_import.js::processImportModule` marks a namespace import
// (`storedFileInfo.isAsPageImport = true`), and `processImportNode` then registers the imported
// declarations under the qualified name it is read by (`Names.Card`), which is the key
// `process_component_class.js` looks up while `isAsPageImport` holds. The child below is reached
// through that qualified name, so the call site has to take the child-construction form.
import * as Names from '../common/Card';

@Entry
@Component
struct NamespaceImportChild {
  build() {
    Column() {
      Names.Card({ title: 'qualified' })
    }
  }
}
