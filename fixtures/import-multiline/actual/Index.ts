if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface MultilineImport_Params {
	text?: string;
}
import { factor, label } from '@normalized:N&&&entry/src/main/ets/pages/Multi&';
import { /* inline */ base } from '@normalized:N&&&entry/src/main/ets/pages/Multi&';
class MultilineImport extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__text = new ObservedPropertySimplePU('', this, 'text');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: MultilineImport_Params) {
		if (params.text !== undefined) {
			this.text = params.text;
		}
	}
	updateStateVars(params: MultilineImport_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__text.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__text.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __text: ObservedPropertySimplePU<string>;
	get text() {
		return this.__text.get();
	}
	set text(newValue: string) {
		this.__text.set(newValue);
	}
	aboutToAppear(): void {
		this.text = String(factor) + label + String(base);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.text);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'MultilineImport';
	}
}
registerNamedRoute(() => new MultilineImport(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
