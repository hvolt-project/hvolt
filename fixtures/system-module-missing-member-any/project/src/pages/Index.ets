
import deviceInfo from "@ohos.deviceInfo";

const level = deviceInfo.sdkApiVersion;
const high = deviceInfo.notThere;

@Entry
@Component
struct BatchProbe20 {
  build() {
    Column() {
      Text("x")
    }
  }
}
