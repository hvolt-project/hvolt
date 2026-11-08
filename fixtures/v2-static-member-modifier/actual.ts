if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
class V2StaticMemberModifier extends ViewV2 {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
		super(parent, elmtId, extraInfo);
		this.value = 1;
		this.finalizeConstruction();
	}
	public resetStateVarsOnReuse(params: Object): void {
		this.value = 1;
	}
	public static count: number = 0;
	private static label: string = 'a';
	static plain: boolean = false;
	@Local public value: number;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${V2StaticMemberModifier.count}`);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(V2StaticMemberModifier.label);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${V2StaticMemberModifier.plain}`);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.value}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'V2StaticMemberModifier';
	}
}
registerNamedRoute(() => new V2StaticMemberModifier(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
