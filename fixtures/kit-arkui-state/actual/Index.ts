if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface KitArkuiState_Params {
	label?: string;
}
import { AppStorageV2 } from '@ohos:arkui.StateManagement';
class ProbeStore {
	value: number = 1;
}
const connected: ProbeStore = AppStorageV2.connect(ProbeStore, 'probe', () => new ProbeStore())!;
class KitArkuiState extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__label = new ObservedPropertySimplePU('state', this, 'label');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: KitArkuiState_Params) {
		if (params.label !== undefined) {
			this.label = params.label;
		}
	}
	updateStateVars(params: KitArkuiState_Params) {}
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
			Text.create(this.label + String(connected.value));
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'KitArkuiState';
	}
}
registerNamedRoute(() => new KitArkuiState(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
