if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface WatchView_Params {
    count?: number;
    label?: string;
}
class WatchView extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__count = new ObservedPropertySimplePU(0, this, "count");
        this.__label = new ObservedPropertySimplePU('zero', this, "label");
        this.setInitiallyProvidedValue(params);
        this.declareWatch("label", this.onCountChange);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: WatchView_Params) {
        if (params.count !== undefined) {
            this.count = params.count;
        }
        if (params.label !== undefined) {
            this.label = params.label;
        }
    }
    updateStateVars(params: WatchView_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__count.purgeDependencyOnElmtId(rmElmtId);
        this.__label.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__count.aboutToBeDeleted();
        this.__label.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __count: ObservedPropertySimplePU<number>;
    get count() {
        return this.__count.get();
    }
    set count(newValue: number) {
        this.__count.set(newValue);
    }
    private __label: ObservedPropertySimplePU<string>;
    get label() {
        return this.__label.get();
    }
    set label(newValue: string) {
        this.__label.set(newValue);
    }
    onCountChange(): void {
        this.label = `${this.count}`;
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(this.label);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "WatchView";
    }
}
registerNamedRoute(() => new WatchView(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
