if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface CommentForms_Params {
	ticks?: number;
}
class CommentForms extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__ticks = new ObservedPropertySimplePU(0, this, "ticks");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: CommentForms_Params) {
        if (params.ticks !== undefined) {
            this.ticks = params.ticks;
        }
    }
    updateStateVars(params: CommentForms_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__ticks.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__ticks.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __ticks: ObservedPropertySimplePU<number>;
    get ticks() {
        return this.__ticks.get();
    }
    set ticks(newValue: number) {
        this.__ticks.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // the box
                Column.create();
                globalThis.Context.animation({ duration: 50 });
                // the box
                Column.width('100%');
                globalThis.Gesture.create(GesturePriority.Low);
                TapGesture.create({ count: 1 });
                TapGesture.onAction(() => {
            this.ticks = 1
          });
                TapGesture.pop();
                globalThis.Gesture.pop();
                globalThis.Context.animation(null);
            }, Column);
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    Text.create('inner');
                }, Text);
                Text.pop();
            // the box
            Column.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "CommentForms";
    }
}
registerNamedRoute(() => new CommentForms(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
