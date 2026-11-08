if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface SheetView_Params {
    shown?: boolean;
}
class SheetView extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__shown = new ObservedPropertySimplePU(false, this, "shown");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: SheetView_Params) {
        if (params.shown !== undefined) {
            this.shown = params.shown;
        }
    }
    updateStateVars(params: SheetView_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__shown.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__shown.aboutToBeDeleted();
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
    panel(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('panel');
        }, Text);
        Text.pop();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
            Column.bindSheet({ value: this.shown, changeEvent: newValue => { this.shown = newValue; } }, { builder: () => {
                    this.panel.call(this);
                } }, { detents: [100] });
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Button.createWithLabel('open');
            Button.onClick(() => {
                this.shown = true;
            });
        }, Button);
        Button.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "SheetView";
    }
}
registerNamedRoute(() => new SheetView(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
