if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface TrailingIndex_Params {}
interface PairProbe_Params {
	slotA?: () => void;
	slotB?: () => void;
}
interface SingleFieldProbe_Params {
	only?: number;
}
interface TrailingProbe_Params {
    slot?: () => void;
    count?: number;
    // TRAILING_PLAIN
    tail?: () => void;
}
function globalBuilder(parent = null) {
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
		Text.create('gb');
	}, Text);
	Text.pop();
}
class TrailingProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.slot = globalBuilder;
        this.__count = new ObservedPropertySimplePU(0
        // TRAILING_PLAIN
        , this, "count");
        this.tail = globalBuilder;
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: TrailingProbe_Params) {
        if (params.slot !== undefined) {
            this.slot = params.slot;
        }
        if (params.count !== undefined) {
            this.count = params.count;
        }
        if (params.tail !== undefined) {
            this.tail = params.tail;
        }
    }
    updateStateVars(params: TrailingProbe_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__count.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__count.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __slot;
    // TRAILING_FIELD
    private __count: ObservedPropertySimplePU<number>;
    get count() {
        return this.__count.get();
    }
    set count(newValue: number) {
        this.__count.set(newValue);
    }
    // TRAILING_PLAIN
    private __tail;
// TRAILING_THEN_LEADING
  // LEADING_AFTER_TRAILING
  onUpdate(): void {
    this.count = 1; // TRAILING_STMT
  }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            this.slot.bind(this)()
            this.tail.bind(this)()
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                Text.create(`${this.count}`);
            }, Text);
            Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class SingleFieldProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__only = new ObservedPropertySimplePU(0, this, "only");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: SingleFieldProbe_Params) {
        if (params.only !== undefined) {
            this.only = params.only;
        }
    }
    updateStateVars(params: SingleFieldProbe_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__only.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__only.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __only: ObservedPropertySimplePU<number>;
    get only() {
        return this.__only.get();
    }
    set only(newValue: number) {
        this.__only.set(newValue);
    }
    // ONLY_FIELD
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.only}`);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class PairProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.slotA = globalBuilder;
        this.slotB = globalBuilder;
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: PairProbe_Params) {
        if (params.slotA !== undefined) {
            this.slotA = params.slotA;
        }
        if (params.slotB !== undefined) {
            this.slotB = params.slotB;
        }
    }
    updateStateVars(params: PairProbe_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __slotA;
// TRAIL_A
  // LEADING_A
  onUpdateA(): void {
    this.slotA()
  }
    private __slotB;
// TRAIL_B
  // LEADING_B
  onUpdateB(): void {
    this.slotB()
  }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            this.slotA.bind(this)()
            this.slotB.bind(this)()
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class TrailingIndex extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: TrailingIndex_Params) {}
	updateStateVars(params: TrailingIndex_Params) {}
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
					let componentCall = new TrailingProbe(this, { tail: globalBuilder }, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 60,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return { tail: globalBuilder };
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
			}, { name: 'TrailingProbe' });
		}
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new SingleFieldProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 61,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
			}, { name: 'SingleFieldProbe' });
		}
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new PairProbe(this, { slotB: globalBuilder }, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 62,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return { slotB: globalBuilder };
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
			}, { name: 'PairProbe' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'TrailingIndex';
	}
}
registerNamedRoute(() => new TrailingIndex(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
