import { MapHolder } from '../common/MapHolder';

// A constructor's type arguments survive the lowering: `new Map<string, string>([])` keeps its
// arguments in the constructor line the reference emits, and so does a plain file's field.
@Entry
@Component
struct MapTypeArgs {
  @StorageLink('mapProfiles') profiles: Map<string, string> = new Map<string, string>([]);
  @State holder: MapHolder = new MapHolder();

  build() {
    Column() {
      Text(`${this.profiles.size + this.holder.total()}`)
    }
  }
}
