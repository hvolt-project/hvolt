if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface AttrForms_Params {
	open?: boolean;
	progress?: number;
}
class AttrForms extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__open = new ObservedPropertySimplePU(false, this, 'open');
		this.__progress = new ObservedPropertySimplePU(0, this, 'progress');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: AttrForms_Params) {
		if (params.open !== undefined) {
			this.open = params.open;
		}
		if (params.progress !== undefined) {
			this.progress = params.progress;
		}
	}
	updateStateVars(params: AttrForms_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__open.purgeDependencyOnElmtId(rmElmtId);
		this.__progress.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__open.aboutToBeDeleted();
		this.__progress.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __open: ObservedPropertySimplePU<boolean>;
	get open() {
		return this.__open.get();
	}
	set open(newValue: boolean) {
		this.__open.set(newValue);
	}
	private __progress: ObservedPropertySimplePU<number>;
	get progress() {
		return this.__progress.get();
	}
	set progress(newValue: number) {
		this.__progress.set(newValue);
	}
	panel(parent = null) {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('panel');
		}, Text);
		Text.pop();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('plain');
			Text.fontSize(16);
			Text.fontColor(Color.Red);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('sheet');
			Text.bindSheet({
				value: this.open,
				changeEvent: (newValue) => {
					this.open = newValue;
				}
			}, { builder: () => {
				this.panel.call(this);
			} }, { detents: [100] });
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
			globalThis.Context.animation({ duration: 100 });
			Column.width('100%');
			globalThis.Gesture.create(GesturePriority.Low);
			PanGesture.create({ fingers: 1 });
			PanGesture.onActionStart(() => {
				this.progress = 1;
			});
			PanGesture.onActionEnd(() => {
				this.progress = 0;
			});
			PanGesture.pop();
			globalThis.Gesture.pop();
			globalThis.Context.animation(null);
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create('drag');
		}, Text);
		Text.pop();
		Column.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'AttrForms';
	}
}
registerNamedRoute(() => new AttrForms(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
