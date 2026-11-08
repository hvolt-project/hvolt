if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
class V2Order extends ViewV2 {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
		super(parent, elmtId, extraInfo);
		this.first = 1;
		this.second = 2;
		this.finalizeConstruction();
	}
	public resetStateVarsOnReuse(params: Object): void {
		this.first = 1;
		this.second = 2;
	}
	@Local first: number;
	helper(): number {
		return this.first;
	}
	@Local second: number;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.first}${this.second}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	tail(): number {
		return 3;
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'V2Order';
	}
}
registerNamedRoute(() => new V2Order(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
