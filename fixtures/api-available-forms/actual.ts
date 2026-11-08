if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface ApiAvailableForms_Params {}
import deviceInfo from '@ohos:deviceInfo';
class ApiAvailableForms extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ApiAvailableForms_Params) {}
	updateStateVars(params: ApiAvailableForms_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			If.create();
			if (deviceInfo.sdkApiVersion >= 11) {
				this.ifElseBranchUpdateFunction(0, () => {
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						Text.create('dot');
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
			if (deviceInfo?.sdkApiVersion >= 11) {
				this.ifElseBranchUpdateFunction(0, () => {
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						Text.create('chain');
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
			if (deviceInfo.distributionOSApiVersion >= 50005) {
				this.ifElseBranchUpdateFunction(0, () => {
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						Text.create('no-suffix');
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
			if (deviceInfo.distributionOSApiVersion >= 50005) {
				this.ifElseBranchUpdateFunction(0, () => {
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						Text.create('suffix');
					}, Text);
					Text.pop();
				});
			} else {
				this.ifElseBranchUpdateFunction(1, () => {});
			}
		}, If);
		If.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${deviceInfo.sdkApiVersion >= 12}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'ApiAvailableForms';
	}
}
registerNamedRoute(() => new ApiAvailableForms(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
