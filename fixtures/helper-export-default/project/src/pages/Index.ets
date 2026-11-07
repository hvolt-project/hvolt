// A helper module (no `@Component` struct) that ends with `export default <identifier>;`.
//
// Files with helpers only take `expand_helper_file` in `src/build/lower/ets_component.rs` —
// the second place the `export default` defer rule lived. The reference keeps the statement
// where the source wrote it, so this fixture pins the in-place position on that path.
import header from '../common/Helpers'

@Entry
@Component
struct HelperExportProbe {
  build() {
    Column() {
      header()
      Text('loud').loud()
    }
  }
}
