import { hilog } from '@kit.PerformanceAnalysisKit';
import { curves } from '@kit.ArkUI';
import { ErrorCode } from '@kit.AbilityKit';
import { KeyAction } from '@kit.InputKit';

const CURVE = curves.cubicBezierCurve(0.1, 0.0, 0.9, 1.0);

@Entry
@Component
struct KitImport {
  aboutToAppear(): void {
    hilog.info(0x0000, 'kit', 'code=%{public}d action=%{public}d', ErrorCode.INVALID_PARAMETER, KeyAction.CANCEL);
  }

  build() {
    Column() {
      Text('kit')
    }
  }
}
