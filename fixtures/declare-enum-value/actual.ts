if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface DeclareEnumValue_Params {
	pressed?: string;
}
import { KeyCode } from '@ohos:multimodalInput.keyCode';
class DeclareEnumValue extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__pressed = new ObservedPropertySimplePU('', this, 'pressed');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: DeclareEnumValue_Params) {
		if (params.pressed !== undefined) {
			this.pressed = params.pressed;
		}
	}
	updateStateVars(params: DeclareEnumValue_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__pressed.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__pressed.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __pressed: ObservedPropertySimplePU<string>;
	get pressed() {
		return this.__pressed.get();
	}
	set pressed(newValue: string) {
		this.__pressed.set(newValue);
	}
	private escapePressed(code: number): boolean {
		return code === KeyCode.KEYCODE_ESCAPE;
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.escapePressed(2070) ? 'esc' : this.pressed}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'DeclareEnumValue';
	}
}
registerNamedRoute(() => new DeclareEnumValue(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
