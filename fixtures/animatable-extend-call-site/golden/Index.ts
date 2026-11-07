if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface AnimatableExtendPage_Params {
    target?: number;
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
class AnimatableExtendPage extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__target = new ObservedPropertySimplePU(80, this, "target");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: AnimatableExtendPage_Params) {
        if (params.target !== undefined) {
            this.target = params.target;
        }
    }
    updateStateVars(params: AnimatableExtendPage_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__target.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__target.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __target: ObservedPropertySimplePU<number>;
    get target() {
        return this.__target.get();
    }
    set target(newValue: number) {
        this.__target.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('x');
            animWidth(this.target, elmtId, isInitialRender, this);
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
