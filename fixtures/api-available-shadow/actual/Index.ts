if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface ApiAvailableShadow_Params {
	probe?: Probe;
}
import deviceInfo from '@ohos:deviceInfo';
// A local class with a method named `apiAvailable`. The reference implementation decides the
// rewrite with the type checker: the callee's type must come from `@ohos.deviceInfo.d.ts` and be
// named `apiAvailable`. This one is not, so the call must stay as written.
class Probe {
  apiAvailable(n: number): boolean {
    return n > 0
  }
}
class ApiAvailableShadow extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.probe = new Probe();
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ApiAvailableShadow_Params) {
		if (params.probe !== undefined) {
			this.probe = params.probe;
		}
	}
	updateStateVars(params: ApiAvailableShadow_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private probe: Probe;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			If.create();
			if (deviceInfo.sdkApiVersion >= 11) {
				this.ifElseBranchUpdateFunction(0, () => {
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						Text.create('device');
					}, Text);
					Text.pop();
				});
			} else {
				this.ifElseBranchUpdateFunction(1, () => {});
			}
		}, If);
		If.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			If.create();
			if (this.probe.apiAvailable(11)) {
				this.ifElseBranchUpdateFunction(0, () => {
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						Text.create('local');
					}, Text);
					Text.pop();
				});
			} else {
				this.ifElseBranchUpdateFunction(1, () => {});
			}
		}, If);
		If.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'ApiAvailableShadow';
	}
}
registerNamedRoute(() => new ApiAvailableShadow(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
