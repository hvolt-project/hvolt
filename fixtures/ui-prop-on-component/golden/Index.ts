if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface PropOnClassProbe_Params {
    linkValue?: number;
}
class PropOnClassProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__linkValue = new SynchedPropertySimpleOneWayPU(params.linkValue, this, "linkValue");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: PropOnClassProbe_Params) {
        if (params.linkValue === undefined) {
            this.__linkValue.set(0);
        }
    }
    updateStateVars(params: PropOnClassProbe_Params) {
        this.__linkValue.reset(params.linkValue);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__linkValue.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__linkValue.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __linkValue: SynchedPropertySimpleOneWayPU<number>;
    get linkValue() {
        return this.__linkValue.get();
    }
    set linkValue(newValue: number) {
        this.__linkValue.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.linkValue}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "PropOnClassProbe";
    }
}
registerNamedRoute(() => new PropOnClassProbe(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
