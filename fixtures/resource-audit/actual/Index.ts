if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface ResourceAudit_Params {
	caption?: string;
}
class ResourceAudit extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__caption = new ObservedPropertySimplePU('res', this, 'caption');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ResourceAudit_Params) {
		if (params.caption !== undefined) {
			this.caption = params.caption;
		}
	}
	updateStateVars(params: ResourceAudit_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__caption.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__caption.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __caption: ObservedPropertySimplePU<string>;
	get caption() {
		return this.__caption.get();
	}
	set caption(newValue: string) {
		this.__caption.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create({ "id": 16777222, "type": 10003, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Image.create({ "id": 16777225, "type": 20000, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
		}, Image);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create({ "id": 125829210, "type": 10001, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.caption);
			Text.fontColor({ "id": 16777223, "type": 10001, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'ResourceAudit';
	}
}
registerNamedRoute(() => new ResourceAudit(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
