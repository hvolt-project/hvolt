if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface BindPopupGlobalBuilder_Params {
	shown?: boolean;
	tips?: string[];
}
function popupContent(label: string, parent = null) {
	const __label__ = label;
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, label = __label__) => {
		Text.create(label);
	}, Text);
	Text.pop();
}
function popupBuilder(ctx: object, tip: string, onClose: () => void, parent = null) {
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
		Column.create();
	}, Column);
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
		Text.create(tip);
	}, Text);
	Text.pop();
	Column.pop();
}
function selfArg(ctx: object, parent = null) {
	const __ctx__ = ctx;
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, ctx = __ctx__) => {
		Text.create('self');
	}, Text);
	Text.pop();
}
function twoArgs(first: string, second: string, parent = null) {
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
		Text.create(first + second);
	}, Text);
	Text.pop();
}
class BindPopupGlobalBuilder extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__shown = new ObservedPropertySimplePU(false, this, 'shown');
		this.__tips = new ObservedPropertyObjectPU(['a', 'b'], this, 'tips');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: BindPopupGlobalBuilder_Params) {
		if (params.shown !== undefined) {
			this.shown = params.shown;
		}
		if (params.tips !== undefined) {
			this.tips = params.tips;
		}
	}
	updateStateVars(params: BindPopupGlobalBuilder_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__shown.purgeDependencyOnElmtId(rmElmtId);
		this.__tips.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__shown.aboutToBeDeleted();
		this.__tips.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __shown: ObservedPropertySimplePU<boolean>;
	get shown() {
		return this.__shown.get();
	}
	set shown(newValue: boolean) {
		this.__shown.set(newValue);
	}
	private __tips: ObservedPropertyObjectPU<string[]>;
	get tips() {
		return this.__tips.get();
	}
	set tips(newValue: string[]) {
		this.__tips.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('probe');
			Text.bindPopup(this.shown, {
				builder: { builder: () => {
					popupContent.call(this, 'hello');
				} },
				onWillDismiss: false
			});
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('arrow-arg');
			Text.bindPopup(this.shown, {
				builder: { builder: () => {
					popupBuilder.call(this, this, this.tips[1], () => {
						Text('x');
					});
				} },
				onWillDismiss: false
			});
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('self-arg');
			Text.bindPopup(this.shown, {
				builder: { builder: () => {
					selfArg.call(this, this);
				} },
				onWillDismiss: false
			});
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('two-args');
			Text.bindPopup(this.shown, {
				builder: { builder: () => {
					twoArgs.call(this, 'a', 'b');
				} },
				onWillDismiss: false
			});
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'BindPopupGlobalBuilder';
	}
}
registerNamedRoute(() => new BindPopupGlobalBuilder(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
