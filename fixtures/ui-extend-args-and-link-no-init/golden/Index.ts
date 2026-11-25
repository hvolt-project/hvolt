if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface LinkProbe_Params {
    noInit?: number;
}
function __Text__big(size: number): void {
    Text.fontSize(size);
}
class LinkProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__noInit = new SynchedPropertySimpleTwoWayPU(params.noInit, this, "noInit");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: LinkProbe_Params) {
    }
    updateStateVars(params: LinkProbe_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__noInit.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__noInit.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __noInit: SynchedPropertySimpleTwoWayPU<number>;
    get noInit() {
        return this.__noInit.get();
    }
    set noInit(newValue: number) {
        this.__noInit.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create("x");
            __Text__big(20);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "LinkProbe";
    }
}
registerNamedRoute(() => new LinkProbe(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
