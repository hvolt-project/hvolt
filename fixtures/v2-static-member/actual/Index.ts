if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
class V2Static extends ViewV2 {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
		super(parent, elmtId, extraInfo);
		this.count = 0;
		this.finalizeConstruction();
	}
	public resetStateVarsOnReuse(params: Object): void {
		this.count = 0;
	}
	static readonly TAG: string = 'stat';
	@Local count: number;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.count}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'V2Static';
	}
}
registerNamedRoute(() => new V2Static(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
