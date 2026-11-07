if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface DollarChild_Params {
	total?: number;
}
interface DollarProbe_Params {
	count?: number;
}
class DollarProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__count = new ObservedPropertySimplePU(1, this, 'count');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: DollarProbe_Params) {
		if (params.count !== undefined) {
			this.count = params.count;
		}
	}
	updateStateVars(params: DollarProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__count.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__count.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __count: ObservedPropertySimplePU<number>;
	get count() {
		return this.__count.get();
	}
	set count(newValue: number) {
		this.__count.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new DollarChild(this, { total: this.__count }, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 8,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return { total: this.count };
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
			}, { name: 'DollarChild' });
		}
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new DollarChild(this, { total: this.__count }, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 9,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return { total: this.count };
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
			}, { name: 'DollarChild' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'DollarProbe';
	}
}
class DollarChild extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__total = new SynchedPropertySimpleTwoWayPU(params.total, this, 'total');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: DollarChild_Params) {}
	updateStateVars(params: DollarChild_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__total.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__total.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __total: SynchedPropertySimpleTwoWayPU<number>;
	get total() {
		return this.__total.get();
	}
	set total(newValue: number) {
		this.__total.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.total}`);
		}, Text);
		Text.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
registerNamedRoute(() => new DollarProbe(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
