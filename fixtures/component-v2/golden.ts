if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface V1Badge_Params {
    label?: string;
}
@ObservedV2
class Model {
    @Trace
    name: string = '';
}
class V2View extends ViewV2 {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
        super(parent, elmtId, extraInfo);
        this.count = 0;
        this.theme = 'light';
        this.finalizeConstruction();
    }
    public resetStateVarsOnReuse(params: Object): void {
        this.count = 0;
        this.theme = 'light';
        this.resetComputed("doubled");
        this.resetMonitorsOnReuse();
    }
    @Local
    count: number;
    @Provider('theme')
    theme: string;
    @Computed
    get doubled(): number {
        return this.count * 2;
    }
    @Monitor('count')
    onCountChanged(mon: IMonitor): void {
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new V2Card(this, { title: 'x', once: 1 }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 20, col: 7 });
                    ViewV2.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            title: 'x',
                            once: 1
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {
                        title: 'x', once: 1
                    });
                }
            }, { name: "V2Card" });
        }
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new V1Badge(this, { label: this.theme }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 21, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            label: this.theme
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {
                        label: this.theme
                    });
                }
            }, { name: "V1Badge" });
        }
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "V2View";
    }
}
class V2Card extends ViewV2 {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
        super(parent, elmtId, extraInfo);
        this.initParam("title", (params && "title" in params) ? params.title : '');
        this.initParam("once", (params && "once" in params) ? params.once : 0);
        this.onTap = "onTap" in params ? params.onTap : () => { };
        this.consumedTheme = '';
        this.finalizeConstruction();
    }
    public resetStateVarsOnReuse(params: Object): void {
        this.resetParam("title", (params && "title" in params) ? params.title : '');
        this.resetParam("once", (params && "once" in params) ? params.once : 0);
        this.onTap = "onTap" in params ? params.onTap : () => { };
        this.resetConsumer("consumedTheme", '');
    }
    @Param
    readonly title: string;
    @Param
    @Once
    once: number;
    @Event
    onTap: () => void;
    @Consumer('theme')
    consumedTheme: string;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(this.title);
        }, Text);
        Text.pop();
    }
    public updateStateVars(params) {
        if (params === undefined) {
            return;
        }
        if ("title" in params) {
            this.updateParam("title", params.title);
        }
        if ("once" in params) {
            this.updateParam("once", params.once);
        }
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class V1Badge extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__label = new SynchedPropertySimpleOneWayPU(params.label, this, "label");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: V1Badge_Params) {
        if (params.label === undefined) {
            this.__label.set('');
        }
    }
    updateStateVars(params: V1Badge_Params) {
        this.__label.reset(params.label);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__label.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__label.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __label: SynchedPropertySimpleOneWayPU<string>;
    get label() {
        return this.__label.get();
    }
    set label(newValue: string) {
        this.__label.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(this.label);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
registerNamedRoute(() => new V2View(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
