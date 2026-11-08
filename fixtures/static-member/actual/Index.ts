if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface StaticProbe_Params {
	count?: number;
}
class StaticProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__count = new ObservedPropertySimplePU(0, this, 'count');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: StaticProbe_Params) {
		if (params.count !== undefined) {
			this.count = params.count;
		}
	}
	updateStateVars(params: StaticProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__count.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__count.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	static readonly TAG: string = 'probe';
	private __count: ObservedPropertySimplePU<number>;
	get count() {
		return this.__count.get();
	}
	set count(newValue: number) {
		this.__count.set(newValue);
	}
	static describe(): string {
		return StaticProbe.TAG;
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.count}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'StaticProbe';
	}
}
registerNamedRoute(() => new StaticProbe(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
