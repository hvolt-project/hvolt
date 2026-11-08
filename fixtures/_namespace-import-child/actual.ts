if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface NamespaceImportChild_Params {}
import * as Names from '@normalized:N&&&entry/src/main/ets/common/Card&';
class NamespaceImportChild extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: NamespaceImportChild_Params) {}
	updateStateVars(params: NamespaceImportChild_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		Names.Card({ title: 'qualified' });
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'NamespaceImportChild';
	}
}
registerNamedRoute(() => new NamespaceImportChild(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
