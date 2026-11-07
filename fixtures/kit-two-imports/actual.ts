if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface KitTwoImports_Params {
	label?: string;
}
import { PersistenceV2 } from '@ohos:arkui.StateManagement';
import type { TypeConstructorWithArgs } from '@ohos:arkui.StateManagement';
import { getPersistence } from '@normalized:N&&&entry/src/main/ets/pages/Plain&';
class ProbeStore {
	value: number = 1;
}
export function restore<T extends object>(type: TypeConstructorWithArgs<T>, factory: () => T): T | undefined {
	return PersistenceV2.connect(type, type.name, factory);
}
class KitTwoImports extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__label = new ObservedPropertySimplePU('two', this, 'label');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: KitTwoImports_Params) {
		if (params.label !== undefined) {
			this.label = params.label;
		}
	}
	updateStateVars(params: KitTwoImports_Params) {}
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
			Text.create(this.label + String(getPersistence(ProbeStore, () => new ProbeStore()) !== undefined));
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'KitTwoImports';
	}
}
registerNamedRoute(() => new KitTwoImports(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
