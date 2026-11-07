if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface CarryGate_Params {
    a?: boolean;
    b?: number;
    c?: boolean;
    /**
     * DOC_B
     */
    d?: number;
    e?: boolean;
    g?: boolean;
}
class CarryGate extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__a = this.initializeConsume("a", "a");
        this.__b = new ObservedPropertySimplePU(0
        // PB
        , this, "b");
        this.__c = this.initializeConsume("c", "c");
        this.d = 1;
        this.__e = this.initializeConsume("e", "e");
        this.__g = this.initializeConsume("g", "g");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: CarryGate_Params) {
        if (params.b !== undefined) {
            this.b = params.b;
        }
        if (params.d !== undefined) {
            this.d = params.d;
        }
    }
    updateStateVars(params: CarryGate_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__a.purgeDependencyOnElmtId(rmElmtId);
        this.__b.purgeDependencyOnElmtId(rmElmtId);
        this.__c.purgeDependencyOnElmtId(rmElmtId);
        this.__e.purgeDependencyOnElmtId(rmElmtId);
        this.__g.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__a.aboutToBeDeleted();
        this.__b.aboutToBeDeleted();
        this.__c.aboutToBeDeleted();
        this.__e.aboutToBeDeleted();
        this.__g.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    // PA
    private __a: ObservedPropertyAbstractPU<boolean
    /**
     * DOC_A
     */
    >;
    get a() {
        return this.__a.get();
    }
    set a(newValue: boolean) {
        this.__a.set(newValue);
    }
    /**
     * DOC_A
     */
    private __b: ObservedPropertySimplePU<number>;
    get b() {
        return this.__b.get();
    }
    set b(newValue: number) {
        this.__b.set(newValue);
    }
    // PB
    private __c: ObservedPropertyAbstractPU<boolean
    /**
     * DOC_B
     */
    >;
    get c() {
        return this.__c.get();
    }
    set c(newValue: boolean) {
        this.__c.set(newValue);
    }
    /**
     * DOC_B
     */
    private d: number;
    // PC
    private __e: ObservedPropertyAbstractPU<boolean
    /**
     * DOC_C
     */
    >;
    get e() {
        return this.__e.get();
    }
    set e(newValue: boolean) {
        this.__e.set(newValue);
    }
    /**
     * DOC_C
     */
    public static f: number = 2;
    // PD
    private __g: ObservedPropertyAbstractPU<boolean
    /**
     * DOC_D
     */
    >;
    get g() {
        return this.__g.get();
    }
    set g(newValue: boolean) {
        this.__g.set(newValue);
    }
    /**
     * DOC_D
     */
    h(): number {
        return 3;
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.b}${this.d}${CarryGate.f}${this.h()}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "CarryGate";
    }
}
registerNamedRoute(() => new CarryGate(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
