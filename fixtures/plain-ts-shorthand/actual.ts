if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface PlainTsShorthandPage_Params {
	note?: string;
}
import { makeBag } from '@normalized:N&&&entry/src/main/ets/util/Pair&';
class PlainTsShorthandPage extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__note = new ObservedPropertySimplePU('', this, 'note');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: PlainTsShorthandPage_Params) {
		if (params.note !== undefined) {
			this.note = params.note;
		}
	}
	updateStateVars(params: PlainTsShorthandPage_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__note.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__note.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __note: ObservedPropertySimplePU<string>;
	get note() {
		return this.__note.get();
	}
	set note(newValue: string) {
		this.__note.set(newValue);
	}
	aboutToAppear(): void {
		this.note = makeBag().destination;
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.note);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'PlainTsShorthandPage';
	}
}
registerNamedRoute(() => new PlainTsShorthandPage(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
