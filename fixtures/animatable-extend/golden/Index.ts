if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface AnimatableExtendPage_Params {
}
function animWidth(width: number, elmtId, isInitialRender, parent): void {
    if (isInitialRender) {
        Text.createAnimatableProperty("animWidth", width, (width: number) => {
            ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
            ViewStackProcessor.GetAndPushFrameNode("Text", elmtId);
            Text.width(width);
            ViewStackProcessor.StopGetAccessRecording();
            parent.finishUpdateFunc(elmtId);
        });
        Text.width(width);
    }
    else {
        Text.updateAnimatableProperty("animWidth", width);
    }
}
function animClear(elmtId, isInitialRender, parent): void {
    if (isInitialRender) {
        Text.createAnimatableProperty("animClear", () => {
            ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
            ViewStackProcessor.GetAndPushFrameNode("Text", elmtId);
            Text.opacity(0);
            ViewStackProcessor.StopGetAccessRecording();
            parent.finishUpdateFunc(elmtId);
        });
        Text.opacity(0);
    }
    else {
        Text.updateAnimatableProperty("animClear");
    }
}
function animBoth(w: number, h: number, elmtId, isInitialRender, parent): void {
    if (isInitialRender) {
        Text.createAnimatableProperty("animBoth", w, h, (w: number, h: number) => {
            ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
            ViewStackProcessor.GetAndPushFrameNode("Text", elmtId);
            Text.width(w);
            Text.height(h);
            ViewStackProcessor.StopGetAccessRecording();
            parent.finishUpdateFunc(elmtId);
        });
        Text.width(w);
        Text.height(h);
    }
    else {
        Text.updateAnimatableProperty("animBoth", w, h);
    }
}
function animEmpty(elmtId, isInitialRender, parent): void {
}
class AnimatableExtendPage extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: AnimatableExtendPage_Params) {
    }
    updateStateVars(params: AnimatableExtendPage_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('x');
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "AnimatableExtendPage";
    }
}
registerNamedRoute(() => new AnimatableExtendPage(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
