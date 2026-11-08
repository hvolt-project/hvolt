if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface BuilderForEach_Params {
}
export function ListBuilder(items: string[], parent = null) {
    const __items__ = items;
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, items = __items__) => {
        ForEach.create();
        const forEachItemGenFunction = _item => {
            const item = _item;
            (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, items = __items__) => {
                Text.create(item);
            }, Text);
            Text.pop();
        };
        (parent ? parent : this).forEachUpdateFunction(elmtId, items, forEachItemGenFunction);
    }, ForEach);
    ForEach.pop();
}
class BuilderForEach extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: BuilderForEach_Params) {
    }
    updateStateVars(params: BuilderForEach_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        ListBuilder.bind(this)(['a', 'b']);
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "BuilderForEach";
    }
}
registerNamedRoute(() => new BuilderForEach(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
