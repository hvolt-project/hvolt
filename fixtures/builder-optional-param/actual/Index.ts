if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface BuilderOptionalParamPage_Params {}
function tag(text?: string, parent = null) {
	const __text__ = text;
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, text = __text__) => {
		Text.create(text ?? 'none');
	}, Text);
	Text.pop();
}
class BuilderOptionalParamPage extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: BuilderOptionalParamPage_Params) {}
	updateStateVars(params: BuilderOptionalParamPage_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		tag.bind(this)('one');
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'BuilderOptionalParamPage';
	}
}
registerNamedRoute(() => new BuilderOptionalParamPage(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
