if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface TypeProbe_Params {
    both?: string | number;
    maybe?: number | undefined;
    literals?: 'a' | 'b';
    single?: 'a';
    kind?: Kind | undefined;
}
enum Kind {
    A = 0,
    B = 1
}
class TypeProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__both = new ObservedPropertySimplePU(1, this, "both");
        this.__maybe = new ObservedPropertyObjectPU(undefined, this, "maybe");
        this.__literals = new ObservedPropertySimplePU('a', this, "literals");
        this.__single = new ObservedPropertyObjectPU('a', this, "single");
        this.__kind = new ObservedPropertySimplePU(undefined, this, "kind");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: TypeProbe_Params) {
        if (params.both !== undefined) {
            this.both = params.both;
        }
        if (params.maybe !== undefined) {
            this.maybe = params.maybe;
        }
        if (params.literals !== undefined) {
            this.literals = params.literals;
        }
        if (params.single !== undefined) {
            this.single = params.single;
        }
        if (params.kind !== undefined) {
            this.kind = params.kind;
        }
    }
    updateStateVars(params: TypeProbe_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__both.purgeDependencyOnElmtId(rmElmtId);
        this.__maybe.purgeDependencyOnElmtId(rmElmtId);
        this.__literals.purgeDependencyOnElmtId(rmElmtId);
        this.__single.purgeDependencyOnElmtId(rmElmtId);
        this.__kind.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__both.aboutToBeDeleted();
        this.__maybe.aboutToBeDeleted();
        this.__literals.aboutToBeDeleted();
        this.__single.aboutToBeDeleted();
        this.__kind.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __both: ObservedPropertySimplePU<string | number>;
    get both() {
        return this.__both.get();
    }
    set both(newValue: string | number) {
        this.__both.set(newValue);
    }
    private __maybe: ObservedPropertyObjectPU<number | undefined>;
    get maybe() {
        return this.__maybe.get();
    }
    set maybe(newValue: number | undefined) {
        this.__maybe.set(newValue);
    }
    private __literals: ObservedPropertySimplePU<'a' | 'b'>;
    get literals() {
        return this.__literals.get();
    }
    set literals(newValue: 'a' | 'b') {
        this.__literals.set(newValue);
    }
    private __single: ObservedPropertyObjectPU<'a'>;
    get single() {
        return this.__single.get();
    }
    set single(newValue: 'a') {
        this.__single.set(newValue);
    }
    private __kind: ObservedPropertySimplePU<Kind | undefined>;
    get kind() {
        return this.__kind.get();
    }
    set kind(newValue: Kind | undefined) {
        this.__kind.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.both}${this.maybe}${this.literals}${this.single}${this.kind}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "TypeProbe";
    }
}
registerNamedRoute(() => new TypeProbe(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
