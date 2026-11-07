if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface PassThroughView_Params {
	count?: number;
}
class PassThroughView extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__count = new ObservedPropertySimplePU(0, this, 'count');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: PassThroughView_Params) {
		if (params.count !== undefined) {
			this.count = params.count;
		}
	}
	updateStateVars(params: PassThroughView_Params) {}
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
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.count}`);
			Text.fontSize(20);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Button.createWithLabel('add');
			Button.onClick(() => {
				this.count += 1;
			});
		}, Button);
		Button.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'PassThroughView';
	}
}
registerNamedRoute(() => new PassThroughView(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
