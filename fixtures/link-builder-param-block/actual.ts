if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface Index_Params {
	count?: number;
	flag?: boolean;
}
interface LinkItem_Params {
	content?: () => void;
	linked?: number;
	copied?: boolean;
}
class LinkItem extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.content = undefined;
		this.__linked = new SynchedPropertySimpleTwoWayPU(params.linked, this, 'linked');
		this.__copied = new SynchedPropertySimpleOneWayPU(params.copied, this, 'copied');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: LinkItem_Params) {
		if (params.content !== undefined) {
			this.content = params.content;
		}
		if (params.copied === undefined) {
			this.__copied.set(false);
		}
	}
	updateStateVars(params: LinkItem_Params) {
		this.__copied.reset(params.copied);
	}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__linked.purgeDependencyOnElmtId(rmElmtId);
		this.__copied.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__linked.aboutToBeDeleted();
		this.__copied.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __content;
	private __linked: SynchedPropertySimpleTwoWayPU<number>;
	get linked() {
		return this.__linked.get();
	}
	set linked(newValue: number) {
		this.__linked.set(newValue);
	}
	private __copied: SynchedPropertySimpleOneWayPU<boolean>;
	get copied() {
		return this.__copied.get();
	}
	set copied(newValue: boolean) {
		this.__copied.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.content.bind(this)();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.linked} ${this.copied}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class Index extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__count = new ObservedPropertySimplePU(0, this, 'count');
		this.__flag = new ObservedPropertySimplePU(false, this, 'flag');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: Index_Params) {
		if (params.count !== undefined) {
			this.count = params.count;
		}
		if (params.flag !== undefined) {
			this.flag = params.flag;
		}
	}
	updateStateVars(params: Index_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__count.purgeDependencyOnElmtId(rmElmtId);
		this.__flag.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__count.aboutToBeDeleted();
		this.__flag.aboutToBeDeleted();
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
	private __flag: ObservedPropertySimplePU<boolean>;
	get flag() {
		return this.__flag.get();
	}
	set flag(newValue: boolean) {
		this.__flag.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new LinkItem(this, {
						linked: this.__count,
						copied: this.flag,
						content: () => {
							this.observeComponentCreation2((elmtId, isInitialRender) => {
								Text.create('body');
							}, Text);
							Text.pop();
						}
					}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 26,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return {
							linked: this.count,
							copied: this.flag,
							content: () => {
								this.observeComponentCreation2((elmtId, isInitialRender) => {
									Text.create('body');
								}, Text);
								Text.pop();
							}
						};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, { copied: this.flag });
				}
			}, { name: 'LinkItem' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'Index';
	}
}
registerNamedRoute(() => new Index(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
