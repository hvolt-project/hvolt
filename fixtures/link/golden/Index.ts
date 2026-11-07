if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface LinkChild_Params {
    shared?: number;
}
interface LinkParent_Params {
    shared?: number;
}
class LinkParent extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__shared = new ObservedPropertySimplePU(0, this, "shared");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: LinkParent_Params) {
        if (params.shared !== undefined) {
            this.shared = params.shared;
        }
    }
    updateStateVars(params: LinkParent_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__shared.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__shared.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __shared: ObservedPropertySimplePU<number>;
    get shared() {
        return this.__shared.get();
    }
    set shared(newValue: number) {
        this.__shared.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new LinkChild(this, { shared: this.__shared }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 8, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            shared: this.shared
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {});
                }
            }, { name: "LinkChild" });
        }
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "LinkParent";
    }
}
class LinkChild extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__shared = new SynchedPropertySimpleTwoWayPU(params.shared, this, "shared");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: LinkChild_Params) {
    }
    updateStateVars(params: LinkChild_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__shared.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__shared.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __shared: SynchedPropertySimpleTwoWayPU<number>;
    get shared() {
        return this.__shared.get();
    }
    set shared(newValue: number) {
        this.__shared.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.shared}`);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
registerNamedRoute(() => new LinkParent(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
