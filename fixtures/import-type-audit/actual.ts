if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface ImportTypeAudit_Params {
	shape?: Shape;
	label?: string;
}
import type { Shape } from './Types';
import { describe } from '@normalized:N&&&entry/src/main/ets/pages/Helpers&';
class ImportTypeAudit extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.shape = { kind: 'circle' };
		this.__label = new ObservedPropertySimplePU(describe(this.shape.kind), this, 'label');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ImportTypeAudit_Params) {
		if (params.shape !== undefined) {
			this.shape = params.shape;
		}
		if (params.label !== undefined) {
			this.label = params.label;
		}
	}
	updateStateVars(params: ImportTypeAudit_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__label.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__label.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private shape: Shape;
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
		return 'ImportTypeAudit';
	}
}
registerNamedRoute(() => new ImportTypeAudit(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
