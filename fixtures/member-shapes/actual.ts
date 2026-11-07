if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface MemberShapes_Params {
	scroller?;
	cancel?: () => void;
	noDefault?: string;
	withDefault?: string;
	extra?: () => void;
	count?: number;
}
class MemberShapes extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.scroller = this.getUIContext();
		this.cancel = undefined;
		this.__noDefault = new SynchedPropertySimpleOneWayPU(params.noDefault, this, 'noDefault');
		this.__withDefault = new SynchedPropertySimpleOneWayPU(params.withDefault, this, 'withDefault');
		this.extra = undefined;
		this.__count = new ObservedPropertySimplePU(0, this, 'count');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: MemberShapes_Params) {
		if (params.scroller !== undefined) {
			this.scroller = params.scroller;
		}
		if (params.cancel !== undefined) {
			this.cancel = params.cancel;
		}
		if (params.withDefault === undefined) {
			this.__withDefault.set('');
		}
		if (params.extra !== undefined) {
			this.extra = params.extra;
		}
		if (params.count !== undefined) {
			this.count = params.count;
		}
	}
	updateStateVars(params: MemberShapes_Params) {
		this.__noDefault.reset(params.noDefault);
		this.__withDefault.reset(params.withDefault);
	}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__noDefault.purgeDependencyOnElmtId(rmElmtId);
		this.__withDefault.purgeDependencyOnElmtId(rmElmtId);
		this.__count.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__noDefault.aboutToBeDeleted();
		this.__withDefault.aboutToBeDeleted();
		this.__count.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private scroller;
	private cancel?: () => void;
	private __noDefault: SynchedPropertySimpleOneWayPU<string>;
	get noDefault() {
		return this.__noDefault.get();
	}
	set noDefault(newValue: string) {
		this.__noDefault.set(newValue);
	}
	private __withDefault: SynchedPropertySimpleOneWayPU<string>;
	get withDefault() {
		return this.__withDefault.get();
	}
	set withDefault(newValue: string) {
		this.__withDefault.set(newValue);
	}
	private __extra?;
	private __count: ObservedPropertySimplePU<number>;
	get count() {
		return this.__count.get();
	}
	set count(newValue: number) {
		this.__count.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.count} ${this.withDefault}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	badge(text: string, parent = null) {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(text);
		}, Text);
		Text.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'MemberShapes';
	}
}
registerNamedRoute(() => new MemberShapes(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
