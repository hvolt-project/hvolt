if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface BranchForms_Params {
	step?: number;
}
class BranchForms extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__step = new ObservedPropertySimplePU(0, this, 'step');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: BranchForms_Params) {
		if (params.step !== undefined) {
			this.step = params.step;
		}
	}
	updateStateVars(params: BranchForms_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__step.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__step.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __step: ObservedPropertySimplePU<number>;
	get step() {
		return this.__step.get();
	}
	set step(newValue: number) {
		this.__step.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			If.create();
			if (this.step === 0) {
				this.ifElseBranchUpdateFunction(0, () => {
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						Text.create('zero');
					}, Text);
					Text.pop();
				});
			} else if (this.step === 1) {
				this.ifElseBranchUpdateFunction(1, () => {
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						If.create();
						if (this.step > 10) {
							this.ifElseBranchUpdateFunction(0, () => {
								this.observeComponentCreation2((elmtId, isInitialRender) => {
									Text.create('big');
								}, Text);
								Text.pop();
							});
						} else {
							this.ifElseBranchUpdateFunction(1, () => {
								this.observeComponentCreation2((elmtId, isInitialRender) => {
									Text.create('small');
								}, Text);
								Text.pop();
							});
						}
					}, If);
					If.pop();
				});
			} else if (this.step === 2) {
				this.ifElseBranchUpdateFunction(2, () => {
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						Row.create();
					}, Row);
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						Text.create('two');
					}, Text);
					Text.pop();
					Row.pop();
				});
			} else {
				this.ifElseBranchUpdateFunction(3, () => {
					this.observeComponentCreation2((elmtId, isInitialRender) => {
						Text.create('many');
					}, Text);
					Text.pop();
				});
			}
		}, If);
		If.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'BranchForms';
	}
}
registerNamedRoute(() => new BranchForms(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
