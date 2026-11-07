if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface ChildView_Params {
	value?: number;
}
interface PropLinkView_Params {
	total?: number;
}
class PropLinkView extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__total = new ObservedPropertySimplePU(0, this, 'total');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: PropLinkView_Params) {
		if (params.total !== undefined) {
			this.total = params.total;
		}
	}
	updateStateVars(params: PropLinkView_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__total.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__total.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __total: ObservedPropertySimplePU<number>;
	get total() {
		return this.__total.get();
	}
	set total(newValue: number) {
		this.__total.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new ChildView(this, { value: this.total }, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 8,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return { value: this.total };
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, { value: this.total });
				}
			}, { name: 'ChildView' });
		}
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new ChildView(this, { value: this.total }, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 9,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return { value: this.total };
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, { value: this.total });
				}
			}, { name: 'ChildView' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'PropLinkView';
	}
}
class ChildView extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__value = new SynchedPropertySimpleOneWayPU(params.value, this, 'value');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ChildView_Params) {
		if (params.value === undefined) {
			this.__value.set(0);
		}
	}
	updateStateVars(params: ChildView_Params) {
		this.__value.reset(params.value);
	}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__value.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__value.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __value: SynchedPropertySimpleOneWayPU<number>;
	get value() {
		return this.__value.get();
	}
	set value(newValue: number) {
		this.__value.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.value}`);
		}, Text);
		Text.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
registerNamedRoute(() => new PropLinkView(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
