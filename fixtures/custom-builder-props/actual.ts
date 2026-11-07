if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface CustomBuilderProps_Params {
	index?: number;
}
class CustomBuilderProps extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__index = new ObservedPropertySimplePU(0, this, 'index');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: CustomBuilderProps_Params) {
		if (params.index !== undefined) {
			this.index = params.index;
		}
	}
	updateStateVars(params: CustomBuilderProps_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__index.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__index.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __index: ObservedPropertySimplePU<number>;
	get index() {
		return this.__index.get();
	}
	set index(newValue: number) {
		this.__index.set(newValue);
	}
	TabBar(parent = null) {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('tab');
			Text.fontSize(12);
		}, Text);
		Text.pop();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Tabs.create();
		}, Tabs);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			TabContent.create(() => {
				this.observeComponentCreation2((elmtId, isInitialRender) => {
					Text.create('first');
				}, Text);
				Text.pop();
			});
			TabContent.tabBar({ builder: () => {
				this.TabBar.call(this);
			} });
		}, TabContent);
		TabContent.pop();
		Tabs.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'CustomBuilderProps';
	}
}
registerNamedRoute(() => new CustomBuilderProps(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
