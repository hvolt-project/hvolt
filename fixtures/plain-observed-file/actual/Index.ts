if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface PlainObservedPage_Params {
	counter?: ObservedCounter;
}
import { ObservedCounter } from '@normalized:N&&&entry/src/main/ets/common/ObservedModel&';
class PlainObservedPage extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__counter = new ObservedPropertyObjectPU(new ObservedCounter(), this, 'counter');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: PlainObservedPage_Params) {
		if (params.counter !== undefined) {
			this.counter = params.counter;
		}
	}
	updateStateVars(params: PlainObservedPage_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__counter.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__counter.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __counter: ObservedPropertyObjectPU<ObservedCounter>;
	get counter() {
		return this.__counter.get();
	}
	set counter(newValue: ObservedCounter) {
		this.__counter.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.counter.total}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'PlainObservedPage';
	}
}
registerNamedRoute(() => new PlainObservedPage(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
