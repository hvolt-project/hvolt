if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface UnusedSource_Params {
	label?: string;
}
class UnusedSource extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__label = new ObservedPropertySimplePU('reachable', this, 'label');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: UnusedSource_Params) {
		if (params.label !== undefined) {
			this.label = params.label;
		}
	}
	updateStateVars(params: UnusedSource_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__label.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__label.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __label: ObservedPropertySimplePU<string>;
	get label() {
		return this.__label.get();
	}
	set label(newValue: string) {
		this.__label.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.label);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'UnusedSource';
	}
}
registerNamedRoute(() => new UnusedSource(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
