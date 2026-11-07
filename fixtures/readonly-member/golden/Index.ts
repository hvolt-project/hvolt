if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ReadonlyProbe_Params {
    ro?: number;
    pr?: number;
    pu?: number;
    st?: number;
    plain?: string;
    plainPrivate?: string;
}
class ReadonlyProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.ro = 2;
        this.pr = 3;
        this.pu = 4;
        this.__st = new ObservedPropertySimplePU(5, this, "st");
        this.plain = 'a';
        this.plainPrivate = 'b';
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ReadonlyProbe_Params) {
        if (params.ro !== undefined) {
            this.ro = params.ro;
        }
        if (params.pr !== undefined) {
            this.pr = params.pr;
        }
        if (params.pu !== undefined) {
            this.pu = params.pu;
        }
        if (params.st !== undefined) {
            this.st = params.st;
        }
        if (params.plain !== undefined) {
            this.plain = params.plain;
        }
        if (params.plainPrivate !== undefined) {
            this.plainPrivate = params.plainPrivate;
        }
    }
    updateStateVars(params: ReadonlyProbe_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__st.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__st.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    readonly ro: number;
    private readonly pr: number;
    public readonly pu: number;
    readonly __st: ObservedPropertySimplePU<number>;
    get st() {
        return this.__st.get();
    }
    set st(newValue: number) {
        this.__st.set(newValue);
    }
    private plain: string;
    private plainPrivate: string;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.ro}-${this.pr}-${this.pu}-${this.st}-${this.plain}-${this.plainPrivate}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "ReadonlyProbe";
    }
}
registerNamedRoute(() => new ReadonlyProbe(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
