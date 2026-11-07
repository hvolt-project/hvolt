if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface MethodProbe_Params {
    off?: number | null;
    started?: boolean;
    value?: number;
    watched?: number;
    renderType?: string;
    customTooltip?: () => void;
}
function globalBuilder(parent = null) {
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
        Text.create('gb');
    }, Text);
    Text.pop();
}
class MethodProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.off = null;
        this.started = false;
        this.__value = new ObservedPropertySimplePU(0, this, "value");
        this.__watched = new SynchedPropertySimpleOneWayPU(params.watched, this, "watched");
        this.__renderType = new SynchedPropertySimpleTwoWayPU(params.renderType, this, "renderType");
        this.customTooltip = globalBuilder;
        this.setInitiallyProvidedValue(params);
        this.declareWatch("watched", this.onCountUpdated);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: MethodProbe_Params) {
        if (params.off !== undefined) {
            this.off = params.off;
        }
        if (params.started !== undefined) {
            this.started = params.started;
        }
        if (params.value !== undefined) {
            this.value = params.value;
        }
        if (params.watched === undefined) {
            this.__watched.set(0);
        }
        if (params.customTooltip !== undefined) {
            this.customTooltip = params.customTooltip;
        }
    }
    updateStateVars(params: MethodProbe_Params) {
        this.__watched.reset(params.watched);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__value.purgeDependencyOnElmtId(rmElmtId);
        this.__watched.purgeDependencyOnElmtId(rmElmtId);
        this.__renderType.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__value.aboutToBeDeleted();
        this.__watched.aboutToBeDeleted();
        this.__renderType.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private off: number | null;
    private started: boolean;
    private __value: ObservedPropertySimplePU<number>;
    get value() {
        return this.__value.get();
    }
    set value(newValue: number) {
        this.__value.set(newValue);
    }
    private __watched: SynchedPropertySimpleOneWayPU<number>;
    get watched() {
        return this.__watched.get();
    }
    set watched(newValue: number) {
        this.__watched.set(newValue);
    }
    private __renderType: SynchedPropertySimpleTwoWayPU<string>;
    get renderType() {
        return this.__renderType.get();
    }
    set renderType(newValue: string) {
        this.__renderType.set(newValue);
    }
    private __customTooltip; // 自定义组件
    onCountUpdated(options: number): void {
        // this.off = null
        // if (!this.off) {
        //   this.started = true
        // }
        this.value = options;
    }
    aboutToAppear(): void {
        // this.started = false
        // this.value = 1
        this.value = this.value + 1;
    }
    aboutToDisappear(): void {
        // this.started = false
        // this.off = null
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            // Text('commented out')
            Text.create(`${this.value}`);
        }, Text);
        // Text('commented out')
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "MethodProbe";
    }
}
registerNamedRoute(() => new MethodProbe(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
