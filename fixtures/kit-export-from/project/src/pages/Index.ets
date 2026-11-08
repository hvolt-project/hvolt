// `process_kit_import.js::ExportSpecifierKitInfo.transform` rewrites a re-export that names a
// Kit into one export declaration per underlying module, with the kit's `bindings` field as the
// exported name (`export { default as curves } from "@native:ohos.curves";`), and
// `ExportStarKitInfo.transform` rewrites `export * from '@kit.X'` into one `export * from
// "@ohos:…"` line per distinct source in that kit (with a WARN that every sub-module loads at
// runtime). The declarations keep their source position.
import { curves } from '@kit.ArkUI';

export { curves } from '@kit.ArkUI';
export * from '@kit.AbilityKit';

@Entry
@Component
struct Index {
  build() {
    Column() {
      Text(`${curves.cubicBezierCurve(0.1, 0.2, 0.3, 0.4).interpolate(0.5)}`)
    }
  }
}
