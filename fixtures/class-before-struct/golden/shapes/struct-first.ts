if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface StructFirst_Params {
    n?: number;
}
export class StructFirst extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__n = new ObservedPropertySimplePU(0, this, "n");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: StructFirst_Params) {
        if (params.n !== undefined) {
            this.n = params.n;
        }
    }
    updateStateVars(params: StructFirst_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__n.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__n.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    // SF_BODY
    private __n: ObservedPropertySimplePU<number>;
    get n() {
        return this.__n.get();
    }
    set n(newValue: number) {
        this.__n.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('sf');
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
// BETWEEN_SF_CA
export class ClassAfter {
    // CA_BODY
    v: number = 2;
}
