if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ObservedHost_Params {
    count?: ItemCount;
    local?: LocalItem;
}
/**
 * Counts what the list holds.
 */
@Observed
export class ItemCount {
    total: number = 0;
}
@Observed
class LocalItem {
    title: string = '';
}
class ObservedHost extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__count = new ObservedPropertyObjectPU(new ItemCount(), this, "count");
        this.__local = new ObservedPropertyObjectPU(new LocalItem(), this, "local");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ObservedHost_Params) {
        if (params.count !== undefined) {
            this.count = params.count;
        }
        if (params.local !== undefined) {
            this.local = params.local;
        }
    }
    updateStateVars(params: ObservedHost_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__count.purgeDependencyOnElmtId(rmElmtId);
        this.__local.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__count.aboutToBeDeleted();
        this.__local.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __count: ObservedPropertyObjectPU<ItemCount>;
    get count() {
        return this.__count.get();
    }
    set count(newValue: ItemCount) {
        this.__count.set(newValue);
    }
    private __local: ObservedPropertyObjectPU<LocalItem>;
    get local() {
        return this.__local.get();
    }
    set local(newValue: LocalItem) {
        this.__local.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.count.total} ${this.local.title}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "ObservedHost";
    }
}
registerNamedRoute(() => new ObservedHost(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
