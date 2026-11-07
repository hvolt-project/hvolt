if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface KitImport_Params {
}
import hilog from "@ohos:hilog";
import curves from "@native:ohos.curves";
import { ErrorCode } from "@ohos:ability.errorCode";
import { Action as KeyAction } from "@ohos:multimodalInput.touchEvent";
const CURVE = curves.cubicBezierCurve(0.1, 0.0, 0.9, 1.0);
class KitImport extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: KitImport_Params) {
    }
    updateStateVars(params: KitImport_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    aboutToAppear(): void {
        hilog.info(0x0000, 'kit', 'code=%{public}d action=%{public}d', ErrorCode.INVALID_PARAMETER, KeyAction.CANCEL);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('kit');
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "KitImport";
    }
}
registerNamedRoute(() => new KitImport(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
