if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface LazyForms_Params {
	source?: BasicDataSource;
}
class BasicDataSource implements IDataSource {
	private listeners: DataChangeListener[] = [];
	private data: string[] = [];
	totalCount(): number {
		return this.data.length;
	}
	getData(index: number): string {
		return this.data[index];
	}
	registerDataChangeListener(listener: DataChangeListener): void {
		this.listeners.push(listener);
	}
	unregisterDataChangeListener(listener: DataChangeListener): void {
		const pos = this.listeners.indexOf(listener);
		if (pos >= 0) {
			this.listeners.splice(pos, 1);
		}
	}
}
class LazyForms extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.source = new BasicDataSource();
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: LazyForms_Params) {
		if (params.source !== undefined) {
			this.source = params.source;
		}
	}
	updateStateVars(params: LazyForms_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private source: BasicDataSource;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			List.create();
		}, List);
		{
			const __lazyForEachItemGenFunction = (_item) => {
				const item = _item;
				{
					const itemCreation2 = (elmtId, isInitialRender) => {
						ListItem.create(() => {}, false);
					};
					const observedDeepRender = () => {
						this.observeComponentCreation2(itemCreation2, ListItem);
						this.observeComponentCreation2((elmtId, isInitialRender) => {
							Text.create(item);
						}, Text);
						Text.pop();
						ListItem.pop();
					};
					observedDeepRender();
				}
			};
			const __lazyForEachItemIdFunc = (item: string) => item;
			LazyForEach.create('1', this, this.source, __lazyForEachItemGenFunction, __lazyForEachItemIdFunc);
			LazyForEach.pop();
		}
		{
			const __lazyForEachItemGenFunction = (_item, index: number) => {
				const item = _item;
				{
					const itemCreation2 = (elmtId, isInitialRender) => {
						ListItem.create(() => {}, false);
					};
					const observedDeepRender = () => {
						this.observeComponentCreation2(itemCreation2, ListItem);
						this.observeComponentCreation2((elmtId, isInitialRender) => {
							Text.create(`${index}${item}`);
						}, Text);
						Text.pop();
						ListItem.pop();
					};
					observedDeepRender();
				}
			};
			LazyForEach.create('1', this, this.source, __lazyForEachItemGenFunction);
			LazyForEach.pop();
		}
		{
			const __lazyForEachItemGenFunction = (_item) => {
				const item = _item;
				{
					const itemCreation2 = (elmtId, isInitialRender) => {
						ListItem.create(() => {}, false);
					};
					const observedDeepRender = () => {
						this.observeComponentCreation2(itemCreation2, ListItem);
						this.observeComponentCreation2((elmtId, isInitialRender) => {
							Text.create(item);
						}, Text);
						Text.pop();
						ListItem.pop();
					};
					observedDeepRender();
				}
			};
			const __lazyForEachItemIdFunc = (item: string, index: number) => `${index}`;
			LazyForEach.create('1', this, this.source, __lazyForEachItemGenFunction, __lazyForEachItemIdFunc);
			LazyForEach.pop();
		}
		List.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'LazyForms';
	}
}
registerNamedRoute(() => new LazyForms(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
