if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface V1Mods_Params {
    visible?: number;
    labeled?: string;
}
class V2Private extends ViewV2 {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
        super(parent, elmtId, extraInfo);
        this.hidden = 3;
        this.shown = 4;
        this.finalizeConstruction();
    }
    public resetStateVarsOnReuse(params: Object): void {
    }
    private hidden: number;
    public shown: number;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new V1Mods(this, {}, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 9, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {};
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {});
                }
            }, { name: "V1Mods" });
        }
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.hidden}${this.shown}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "V2Private";
    }
}
class V1Mods extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.visible = 5;
        this.__labeled = new SynchedPropertySimpleOneWayPU(params.labeled, this, "labeled");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: V1Mods_Params) {
        if (params.visible !== undefined) {
            this.visible = params.visible;
        }
        if (params.labeled === undefined) {
            this.__labeled.set('p');
        }
    }
    updateStateVars(params: V1Mods_Params) {
        this.__labeled.reset(params.labeled);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__labeled.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__labeled.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    public visible: number;
    private __labeled: SynchedPropertySimpleOneWayPU<string>;
    get labeled() {
        return this.__labeled.get();
    }
    set labeled(newValue: string) {
        this.__labeled.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.visible}${this.labeled}`);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
registerNamedRoute(() => new V2Private(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
