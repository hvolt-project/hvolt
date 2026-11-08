if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
if (PUV2ViewBase.contextStack === undefined) {
	Reflect.set(PUV2ViewBase, 'contextStack', []);
}
class V2LocalBuilder extends ViewV2 {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
		super(parent, elmtId, extraInfo);
		this.label = 'l';
		this.finalizeConstruction();
	}
	public resetStateVarsOnReuse(params: Object): void {
		this.label = 'l';
	}
	@Local label: string;
	chip = (t: string) => {
		const parent = PUV2ViewBase.contextStack && PUV2ViewBase.contextStack.length ? PUV2ViewBase.contextStack[PUV2ViewBase.contextStack.length - 1] : null;
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(t);
		}, Text);
		Text.pop();
	};
	initialRender() {
		PUV2ViewBase.contextStack && PUV2ViewBase.contextStack.push(this);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.chip.bind(this)(this.label);
		Column.pop();
		PUV2ViewBase.contextStack && PUV2ViewBase.contextStack.pop();
	}
	rerender() {
		PUV2ViewBase.contextStack && PUV2ViewBase.contextStack.push(this);
		this.updateDirtyElements();
		PUV2ViewBase.contextStack && PUV2ViewBase.contextStack.pop();
	}
	static getEntryName(): string {
		return 'V2LocalBuilder';
	}
}
registerNamedRoute(() => new V2LocalBuilder(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
