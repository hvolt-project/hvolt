if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface HelperView_Params {
	label?: string;
}
class HelperView extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__label = new SynchedPropertySimpleOneWayPU(params.label, this, 'label');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: HelperView_Params) {
		if (params.label === undefined) {
			this.__label.set('');
		}
	}
	updateStateVars(params: HelperView_Params) {
		this.__label.reset(params.label);
	}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__label.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__label.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __label: SynchedPropertySimpleOneWayPU<string>;
	get label() {
		return this.__label.get();
	}
	set label(newValue: string) {
		this.__label.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.label);
		}, Text);
		Text.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class MixedView extends ViewV2 {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
		super(parent, elmtId, extraInfo);
		this.count = 0;
		this.finalizeConstruction();
	}
	public resetStateVarsOnReuse(params: Object): void {
		this.count = 0;
	}
	@Local count: number;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new HelperView(this, { label: `${this.count}` }, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 19,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return { label: `${this.count}` };
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, { label: `${this.count}` });
				}
			}, { name: 'HelperView' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'MixedView';
	}
}
registerNamedRoute(() => new MixedView(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
