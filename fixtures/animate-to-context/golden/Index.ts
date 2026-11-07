if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface AnimatePage_Params {
    span?: number;
}
class AnimatePage extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__span = new ObservedPropertySimplePU(10, this, "span");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: AnimatePage_Params) {
        if (params.span !== undefined) {
            this.span = params.span;
        }
    }
    updateStateVars(params: AnimatePage_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__span.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__span.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __span: ObservedPropertySimplePU<number>;
    get span() {
        return this.__span.get();
    }
    set span(newValue: number) {
        this.__span.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('animate');
        }, Text);
        Text.pop();
        Column.pop();
    }
    aboutToAppear(): void {
        Context.animateTo({ duration: 100 }, () => {
            this.span = 20;
        });
        Context.animateToImmediately({ duration: 100 }, () => {
            this.span = 30;
        });
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "AnimatePage";
    }
}
registerNamedRoute(() => new AnimatePage(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
