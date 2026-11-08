if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface ImportForms_Params {
	summary?: string;
	seg?: Segment;
}
import Shape, { Area } from '@normalized:N&&&entry/src/main/ets/pages/Forms&';
import type { Segment } from '@normalized:N&&&entry/src/main/ets/pages/Forms&';
class ImportForms extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__summary = new ObservedPropertySimplePU('', this, 'summary');
		this.seg = { from: 1 };
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ImportForms_Params) {
		if (params.summary !== undefined) {
			this.summary = params.summary;
		}
		if (params.seg !== undefined) {
			this.seg = params.seg;
		}
	}
	updateStateVars(params: ImportForms_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__summary.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__summary.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __summary: ObservedPropertySimplePU<string>;
	get summary() {
		return this.__summary.get();
	}
	set summary(newValue: string) {
		this.__summary.set(newValue);
	}
	private seg: Segment;
	aboutToAppear(): void {
		this.summary = Shape.label + String(Area) + Widget.tag + String(this.seg.from);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.summary);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'ImportForms';
	}
}
export { Area };
export { Area as AreaAlias } from '@normalized:N&&&entry/src/main/ets/pages/Forms&';
export default class Widget {
	static readonly tag: string = 'widget';
}
registerNamedRoute(() => new ImportForms(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
