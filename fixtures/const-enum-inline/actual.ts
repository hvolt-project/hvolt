if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface ConstEnumInline_Params {
	n?: number;
	base?: number;
}
// `const enum` 的成员访问在参考实现里被内联成常量；普通 `enum` 不内联。
// 本 fixture 钉住每一类形态的发射值。
const enum ConstKind {
  A = 1,
  B = 2
}
const enum ConstStr {
	X = 'x',
	Y = 'y'
}
const enum ConstMix {
	A = 0,
	B = 5,
	C = 6
}
enum PlainKind {
	P = 1,
	Q = 2
}
// 普通 enum 的**声明**也按常量值重写（B = A * 2 → B = 2），但成员访问不内联。
enum PlainExpr {
  A = 1,
  B = 2
}
const enum ConstHex {
	H = 16
}
const enum ConstNeg {
	N = -1
}
// 成员用作**类型**时，参考是否也把它换成字面量？（值是 `ConstKind.A`，类型也是）
let typedValue: ConstMix.A = 0
// 成员作类型：参数表 / 返回类型 / 联合类型（ConstMix 的成员全是字面量，才允许这么写）
function takesMember(a: ConstMix.A): number {
  return a
}
function returnsMember(): ConstMix.C {
	return 6;
}
type MemberUnion = ConstMix.A | ConstMix.C;
class ConstEnumInline extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__n = new ObservedPropertySimplePU(0, this, 'n');
		this.base = 2;
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ConstEnumInline_Params) {
		if (params.n !== undefined) {
			this.n = params.n;
		}
		if (params.base !== undefined) {
			this.base = params.base;
		}
	}
	updateStateVars(params: ConstEnumInline_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__n.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__n.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __n: ObservedPropertySimplePU<number>;
	get n() {
		return this.__n.get();
	}
	set n(newValue: number) {
		this.__n.set(newValue);
	}
	private base: number;
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('const-number');
			Text.fontSize(1);
			Text.layoutWeight(2);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('const-string');
			Text.width('x');
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('const-mix');
			Text.fontSize(0);
			Text.layoutWeight(6);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('plain-enum');
			Text.fontSize(PlainKind.P);
			Text.layoutWeight(PlainKind.Q);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('plain-expr');
			Text.layoutWeight(PlainExpr.B);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('const-hex');
			Text.layoutWeight(16);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('const-neg');
			Text.layoutWeight(-1);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('state');
			Text.fontSize(this.n);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'ConstEnumInline';
	}
}
registerNamedRoute(() => new ConstEnumInline(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
