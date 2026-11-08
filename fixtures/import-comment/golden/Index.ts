if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ImportComment_Params {
    value?: number;
}
import { 
// how much to scale by
SCALE, 
// how much to scale by
OFFSET, } from "@normalized:N&&&entry/src/main/ets/pages/Helper&";
class ImportComment extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__value = new ObservedPropertySimplePU(0, this, "value");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ImportComment_Params) {
        if (params.value !== undefined) {
            this.value = params.value;
        }
    }
    updateStateVars(params: ImportComment_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__value.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__value.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __value: ObservedPropertySimplePU<number>;
    get value() {
        return this.__value.get();
    }
    set value(newValue: number) {
        this.__value.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.value * SCALE + OFFSET}`);
            Text.fontSize(16);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "ImportComment";
    }
}
registerNamedRoute(() => new ImportComment(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
