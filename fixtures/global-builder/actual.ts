if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface GlobalBuilderPage_Params {}
function badge(text: string, parent = null) {
	const __text__ = text;
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, text = __text__) => {
		Text.create(text);
		Text.fontSize(12);
	}, Text);
	Text.pop();
}
export function pair(left: string, right: string, parent = null) {
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
		Row.create();
	}, Row);
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
		Text.create(left);
	}, Text);
	Text.pop();
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
		Text.create(right);
	}, Text);
	Text.pop();
	Row.pop();
}
class GlobalBuilderPage extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: GlobalBuilderPage_Params) {}
	updateStateVars(params: GlobalBuilderPage_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		badge.bind(this)('one');
		pair.bind(this)('two', 'three');
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'GlobalBuilderPage';
	}
}
registerNamedRoute(() => new GlobalBuilderPage(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
