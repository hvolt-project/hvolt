if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface Index_Params {}
interface GlobalOnlyProbe_Params {}
interface GlobalStyleShadowProbe_Params {}
interface StylesScopeProbe_Params {}
interface MixOrderProbe_Params {
	dim?: number;
	span?: number;
}
interface EnumMechanismProbe_Params {}
interface EnumProbe_Params {}
interface FormProbe_Params {
	plain?: number;
}
interface ModuleVarProbe_Params {}
interface BuilderProbe_Params {
	footer?: () => void;
}
interface StyleProbe_Params {
	span?: number;
}
interface CallAndTernaryProbe_Params {
	delay?: number;
	big?: boolean;
}
interface PlainMemberProbe_Params {
	delay?: number;
	count?: number;
}
const GAP: number = 8;
function getColor(value: number): ResourceColor {
	return Color.Red;
}
class Shadow {
	static readonly GAP: number = 6;
}
enum LocalKind {
	A = 1,
	B = 2
}
class PlainMemberProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__delay = new ObservedPropertySimplePU(0, this, 'delay');
		this.count = 3;
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: PlainMemberProbe_Params) {
		if (params.delay !== undefined) {
			this.delay = params.delay;
		}
		if (params.count !== undefined) {
			this.count = params.count;
		}
	}
	updateStateVars(params: PlainMemberProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__delay.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__delay.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {
		this.__delay.updateElmtId(oldElmtId, newElmtId);
	}
	private __delay: ObservedPropertySimplePU<number>;
	get delay() {
		return this.__delay.get();
	}
	set delay(newValue: number) {
		this.__delay.set(newValue);
	}
	private count: number;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('plain');
			Text.margin(GAP);
			if (isInitialRender) {
				Text.fontSize(this.count);
			}
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class CallAndTernaryProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__delay = new ObservedPropertySimplePU(0, this, 'delay');
		this.__big = new ObservedPropertySimplePU(false, this, 'big');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: CallAndTernaryProbe_Params) {
		if (params.delay !== undefined) {
			this.delay = params.delay;
		}
		if (params.big !== undefined) {
			this.big = params.big;
		}
	}
	updateStateVars(params: CallAndTernaryProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__delay.purgeDependencyOnElmtId(rmElmtId);
		this.__big.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__delay.aboutToBeDeleted();
		this.__big.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {
		this.__delay.updateElmtId(oldElmtId, newElmtId);
		this.__big.updateElmtId(oldElmtId, newElmtId);
	}
	private __delay: ObservedPropertySimplePU<number>;
	get delay() {
		return this.__delay.get();
	}
	set delay(newValue: number) {
		this.__delay.set(newValue);
	}
	private __big: ObservedPropertySimplePU<boolean>;
	get big() {
		return this.__big.get();
	}
	set big(newValue: boolean) {
		this.__big.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('call');
			Text.fontColor(getColor(this.delay));
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('control');
			Text.fontColor(getColor(3));
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('ternary');
			Text.fontSize(this.big ? 20 : 12);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('binary');
			Text.fontSize(12 + this.delay);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class StyleProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__span = new ObservedPropertySimplePU(10, this, 'span');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: StyleProbe_Params) {
		if (params.span !== undefined) {
			this.span = params.span;
		}
	}
	updateStateVars(params: StyleProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__span.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__span.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {
		this.__span.updateElmtId(oldElmtId, newElmtId);
	}
	private __span: ObservedPropertySimplePU<number>;
	get span() {
		return this.__span.get();
	}
	set span(newValue: number) {
		this.__span.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('styled');
			Text.width(this.span);
			if (isInitialRender) {
				Text.padding(5);
				Text.height(20);
				Text.width(40);
			}
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('styled-plain');
			if (isInitialRender) {
				Text.padding(5);
				Text.height(20);
				Text.width(40);
			}
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('styled-pair');
			if (isInitialRender) {
				Text.width(50);
				Text.height(30);
			}
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class BuilderProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.footer = undefined;
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: BuilderProbe_Params) {
		if (params.footer !== undefined) {
			this.footer = params.footer;
		}
	}
	updateStateVars(params: BuilderProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {}
	private __footer;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('host');
			if (isInitialRender) {
				Text.fontSize(12);
			}
		}, Text);
		Text.pop();
		this.footer.bind(this)();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
// (2) build() 内的局部变量写不出来：编译器报
//   10905210 In an '@Entry' decorated component, the 'build' method can have only one root node
// 退而求其次测「模块级 let（运行期变量、无 this）」。
let runtimeWidth: number = 10
class ModuleVarProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ModuleVarProbe_Params) {}
	updateStateVars(params: ModuleVarProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('module-var');
			Text.width(runtimeWidth);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class FormProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.plain = 4;
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: FormProbe_Params) {
		if (params.plain !== undefined) {
			this.plain = params.plain;
		}
	}
	updateStateVars(params: FormProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {}
	private plain: number;
	widthOf(): number {
		return 10;
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('static-member');
			Text.fontSize(Shadow.GAP);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('object-literal');
			if (isInitialRender) {
				Text.margin({ top: 2 });
			}
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('object-identifier');
			Text.margin({ top: GAP });
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('object-member');
			if (isInitialRender) {
				Text.margin({ top: this.plain });
			}
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('unary');
			Text.layoutWeight(-1);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('binary-const');
			Text.fontSize(2 * 3);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('template');
			Text.width(`${this.plain}%`);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('call-this');
			Text.width(this.widthOf());
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class EnumProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: EnumProbe_Params) {}
	updateStateVars(params: EnumProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('sdk-enum');
			if (isInitialRender) {
				Text.textAlign(TextAlign.End);
			}
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('local-enum');
			if (isInitialRender) {
				Text.fontSize(LocalKind.A);
			}
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('local-class-static');
			Text.fontSize(Shadow.GAP);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('enum-mix');
			Text.width(Shadow.GAP);
			if (isInitialRender) {
				Text.fontSize(LocalKind.A);
				Text.textAlign(TextAlign.End);
			}
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
// ===== A：枚举成员 / 类静态成员的机制 =====
const enum ConstKind {
  A = 1,
  B = 2
}
namespace NsHost {
	export const X: number = 1;
}
class StaticFields {
	static readonly READONLY_GAP: number = 3;
	static mut: number = 4;
}
const kindVar: LocalKind = LocalKind.A;
// 无类型注解：TS 推断成字面量类型 1（不是 enum member，也不是 number）
const AUTO = 1
class EnumMechanismProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: EnumMechanismProbe_Params) {}
	updateStateVars(params: EnumMechanismProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('const-enum');
			if (isInitialRender) {
				Text.fontSize(1);
			}
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('as-cast');
			Text.fontSize(LocalKind.A as number);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('namespace');
			Text.fontSize(NsHost.X);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('enum-var');
			Text.fontSize(kindVar);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('const-auto');
			Text.fontSize(AUTO);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('static-readonly');
			Text.fontSize(StaticFields.READONLY_GAP);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('static-mutable');
			Text.fontSize(StaticFields.mut);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class MixOrderProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__dim = new ObservedPropertySimplePU(12, this, 'dim');
		this.__span = new ObservedPropertySimplePU(10, this, 'span');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: MixOrderProbe_Params) {
		if (params.dim !== undefined) {
			this.dim = params.dim;
		}
		if (params.span !== undefined) {
			this.span = params.span;
		}
	}
	updateStateVars(params: MixOrderProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__dim.purgeDependencyOnElmtId(rmElmtId);
		this.__span.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__dim.aboutToBeDeleted();
		this.__span.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {
		this.__dim.updateElmtId(oldElmtId, newElmtId);
		this.__span.updateElmtId(oldElmtId, newElmtId);
	}
	private __dim: ObservedPropertySimplePU<number>;
	get dim() {
		return this.__dim.get();
	}
	set dim(newValue: number) {
		this.__dim.set(newValue);
	}
	private __span: ObservedPropertySimplePU<number>;
	get span() {
		return this.__span.get();
	}
	set span(newValue: number) {
		this.__span.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('mix1');
			Text.fontSize(this.dim);
			Text.margin(GAP);
			if (isInitialRender) {
				Text.height(80);
				Text.width(70);
				Text.width(40);
				Text.height(20);
				Text.padding(5);
			}
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('mix2');
			if (isInitialRender) {
				Text.backgroundColor(Color.Blue);
				Text.margin(2);
				Text.padding(1);
				Text.height(80);
				Text.width(70);
				Text.width(90);
			}
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('mix3');
			Text.fontColor(getColor(this.dim));
			Text.fontColor(getColor(this.span));
			if (isInitialRender) {
				Text.height(80);
				Text.width(70);
			}
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class StylesScopeProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: StylesScopeProbe_Params) {}
	updateStateVars(params: StylesScopeProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('scope');
			if (isInitialRender) {
				Text.height(12);
				Text.width(11);
			}
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class GlobalStyleShadowProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: GlobalStyleShadowProbe_Params) {}
	updateStateVars(params: GlobalStyleShadowProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('global-shadowed');
			if (isInitialRender) {
				Text.height(22);
				Text.width(21);
			}
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class GlobalOnlyProbe extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: GlobalOnlyProbe_Params) {}
	updateStateVars(params: GlobalOnlyProbe_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('global-only');
			if (isInitialRender) {
				Text.height(98);
				Text.width(99);
			}
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class Index extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: Index_Params) {}
	updateStateVars(params: Index_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('PlainMemberProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new PlainMemberProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 281,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'PlainMemberProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('CallAndTernaryProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new CallAndTernaryProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 282,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'CallAndTernaryProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('StyleProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new StyleProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 283,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'StyleProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('ModuleVarProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new ModuleVarProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 284,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'ModuleVarProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('FormProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new FormProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 285,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'FormProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('EnumMechanismProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new EnumMechanismProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 286,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'EnumMechanismProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('MixOrderProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new MixOrderProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 287,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'MixOrderProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('StylesScopeProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new StylesScopeProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 288,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'StylesScopeProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('GlobalStyleShadowProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new GlobalStyleShadowProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 289,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'GlobalStyleShadowProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('GlobalOnlyProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new GlobalOnlyProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 290,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'GlobalOnlyProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('EnumProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new EnumProbe(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 291,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'EnumProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({});
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			__Recycle__.create();
		}, __Recycle__);
		{
			this.observeRecycleComponentCreation('BuilderProbe', (elmtId, isInitialRender, recycleNode = null) => {
				ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
				if (isInitialRender) {
					let componentCall = recycleNode ? recycleNode : new BuilderProbe(this, { footer: () => {
						this.observeComponentCreation2((elmtId, isInitialRender) => {
							Text.create('inside');
							Text.fontSize(11);
						}, Text);
						Text.pop();
					} }, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 292,
						col: 7
					});
					ViewPU.createRecycle(componentCall, recycleNode !== null, 'BuilderProbe', () => {
						if (recycleNode && typeof recycleNode.aboutToReuseInternal === 'function') {
							recycleNode.aboutToReuseInternal();
						} else {
							if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === 'function') {
								recycleNode.aboutToReuse({ footer: () => {
									this.observeComponentCreation2((elmtId, isInitialRender) => {
										Text.create('inside');
										Text.fontSize(11);
									}, Text);
									Text.pop();
								} });
							}
							recycleNode.rerender();
						}
					});
					let paramsLambda = () => {
						return { footer: () => {
							this.observeComponentCreation2((elmtId, isInitialRender) => {
								Text.create('inside');
								Text.fontSize(11);
							}, Text);
							Text.pop();
						} };
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
				ViewStackProcessor.StopGetAccessRecording();
			});
		}
		__Recycle__.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'Index';
	}
}
registerNamedRoute(() => new Index(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
