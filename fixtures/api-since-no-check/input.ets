
import { launcherBundleManager } from "@kit.AbilityKit";

function read(): number {
  const info = launcherBundleManager.getLauncherAbilityInfoSync("com.example.demo", 100);
  return info.length;
}

@Entry
@Component
struct BatchProbe57 {
  build() {
    Column() {
      Text("x")
    }
  }
}
