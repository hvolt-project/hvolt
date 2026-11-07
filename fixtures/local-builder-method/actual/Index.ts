if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface LocalBuilderView_Params {
	count?: number;
}
// Regression cover for three spelling-based shortcuts:
//  1. `export struct` declarations must be recognised as structs;
//  2. a builder method may be lowercase and take parameters;
//  3. a comment inside a child's argument list must not swallow the generated line.
function isHex(hex: string): boolean {
  return /^[0-9A-F]{2}$/.test(hex);
}
export class LocalBuilderView extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__count = new ObservedPropertySimplePU(0, this, 'count');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: LocalBuilderView_Params) {
		if (params.count !== undefined) {
			this.count = params.count;
		}
	}
	updateStateVars(params: LocalBuilderView_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__count.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__count.aboutToBeDeleted();
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
	dialogContent(data: string, parent = null) {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			ForEach.create();
			const forEachItemGenFunction = (_item) => {
				const item = _item;
				this.observeComponentCreation2((elmtId, isInitialRender) => {
					Text.create(`${item}`);
				}, Text);
				Text.pop();
			};
			this.forEachUpdateFunction(elmtId, [data], forEachItemGenFunction);
		}, ForEach);
		ForEach.pop();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(isHex('FF') ? 'ok' : 'bad');
		}, Text);
		Text.pop();
		this.dialogContent.bind(this)(`${this.count}`);
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'LocalBuilderView';
	}
}
registerNamedRoute(() => new LocalBuilderView(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
