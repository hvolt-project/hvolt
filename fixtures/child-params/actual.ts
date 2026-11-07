if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface LinkChild_Params {
	total?: number;
}
interface PropChild_Params {
	value?: number;
	text?: string;
}
interface ChildParams_Params {
	count?: number;
	label?: string;
}
class ChildParams extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__count = new ObservedPropertySimplePU(3, this, 'count');
		this.__label = new ObservedPropertySimplePU('n', this, 'label');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ChildParams_Params) {
		if (params.count !== undefined) {
			this.count = params.count;
		}
		if (params.label !== undefined) {
			this.label = params.label;
		}
	}
	updateStateVars(params: ChildParams_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__count.purgeDependencyOnElmtId(rmElmtId);
		this.__label.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__count.aboutToBeDeleted();
		this.__label.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __count: ObservedPropertySimplePU<number>;
	get count() {
		return this.__count.get();
	}
	set count(newValue: number) {
		this.__count.set(newValue);
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
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new PropChild(this, {
						value: this.count,
						text: this.label
					}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 9,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return {
							value: this.count,
							text: this.label
						};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {
						value: this.count,
						text: this.label
					});
				}
			}, { name: 'PropChild' });
		}
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new LinkChild(this, { total: this.__count }, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 10,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return { total: this.count };
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
			}, { name: 'LinkChild' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'ChildParams';
	}
}
class PropChild extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__value = new SynchedPropertySimpleOneWayPU(params.value, this, 'value');
		this.__text = new SynchedPropertySimpleOneWayPU(params.text, this, 'text');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: PropChild_Params) {
		if (params.value === undefined) {
			this.__value.set(0);
		}
		if (params.text === undefined) {
			this.__text.set('');
		}
	}
	updateStateVars(params: PropChild_Params) {
		this.__value.reset(params.value);
		this.__text.reset(params.text);
	}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__value.purgeDependencyOnElmtId(rmElmtId);
		this.__text.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__value.aboutToBeDeleted();
		this.__text.aboutToBeDeleted();
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
	private __text: SynchedPropertySimpleOneWayPU<string>;
	get text() {
		return this.__text.get();
	}
	set text(newValue: string) {
		this.__text.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.value}${this.text}`);
		}, Text);
		Text.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class LinkChild extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__total = new SynchedPropertySimpleTwoWayPU(params.total, this, 'total');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: LinkChild_Params) {}
	updateStateVars(params: LinkChild_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__total.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__total.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __total: SynchedPropertySimpleTwoWayPU<number>;
	get total() {
		return this.__total.get();
	}
	set total(newValue: number) {
		this.__total.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.total}`);
		}, Text);
		Text.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
registerNamedRoute(() => new ChildParams(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
