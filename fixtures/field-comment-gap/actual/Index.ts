if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface GapIndex_Params {}
interface GapProbe_Params {
	a?: number;
	b?: string;
	c?: string;
	d?: number;
	e?: string;
}
class GapProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__a = new ObservedPropertySimplePU(1
        // NEXT_A
        , this, "a");
        this.__b = new SynchedPropertySimpleOneWayPU(params.b
        // NEXT_B
        , this, "b");
        this.c = '3';
        this.__d = new ObservedPropertySimplePU(4
        // NEXT_D
        , this, "d");
        this.__e = new SynchedPropertySimpleOneWayPU(params.e, this, "e");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: GapProbe_Params) {
        if (params.a !== undefined) {
            this.a = params.a;
        }
        if (params.b === undefined) {
            this.__b.set('2');
        }
        if (params.c !== undefined) {
            this.c = params.c;
        }
        if (params.d !== undefined) {
            this.d = params.d;
        }
        if (params.e === undefined) {
            this.__e.set('5');
        }
    }
    updateStateVars(params: GapProbe_Params) {
        this.__b.reset(params.b);
        this.__e.reset(params.e);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__a.purgeDependencyOnElmtId(rmElmtId);
        this.__b.purgeDependencyOnElmtId(rmElmtId);
        this.__d.purgeDependencyOnElmtId(rmElmtId);
        this.__e.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__a.aboutToBeDeleted();
        this.__b.aboutToBeDeleted();
        this.__d.aboutToBeDeleted();
        this.__e.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __a: ObservedPropertySimplePU<number>;
    get a() {
        return this.__a.get();
    }
    set a(newValue: number) {
        this.__a.set(newValue);
    }
    // NEXT_A
    private __b: SynchedPropertySimpleOneWayPU<string>;
    get b() {
        return this.__b.get();
    }
    set b(newValue: string) {
        this.__b.set(newValue);
    }
    // NEXT_B
    private c: string;
    // NEXT_C
    private __d: ObservedPropertySimplePU<number>;
    get d() {
        return this.__d.get();
    }
    set d(newValue: number) {
        this.__d.set(newValue);
    }
    // NEXT_D
    private __e: SynchedPropertySimpleOneWayPU<string>;
    get e() {
        return this.__e.get();
    }
    set e(newValue: string) {
        this.__e.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                Text.create(`${this.a}${this.b}${this.c}${this.d}${this.e}`);
            }, Text);
            Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class GapIndex extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: GapIndex_Params) {}
	updateStateVars(params: GapIndex_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new GapProbe(this, { e: 'x' }, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 25,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return { e: 'x' };
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, { e: 'x' });
				}
			}, { name: 'GapProbe' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'GapIndex';
	}
}
registerNamedRoute(() => new GapIndex(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
