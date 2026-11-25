
import { resourceManager } from "@kit.LocalizationKit";

function read(): number {
  const ctx = getContext();
  const mgr: resourceManager.ResourceManager = ctx.resourceManager;
  return mgr.nope;
}

@Entry
@Component
struct BatchProbe60 {
  build() {
    Column() {
      Text("getContext()")
    }
  }
}
