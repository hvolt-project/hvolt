if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface BindSheetTitleProbe_Params {
    shown?: boolean;
}
class BindSheetTitleProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__shown = new ObservedPropertySimplePU(false, this, "shown");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: BindSheetTitleProbe_Params) {
        if (params.shown !== undefined) {
            this.shown = params.shown;
        }
    }
    updateStateVars(params: BindSheetTitleProbe_Params) {
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
    sheetBody(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('body');
        }, Text);
        Text.pop();
    }
    sheetTitle(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('title');
        }, Text);
        Text.pop();
    }
    BindSheetTitleCustomBuilder(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('named title');
        }, Text);
        Text.pop();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            // 1 title as an inline arrow
            Column.create();
            // 1 title as an inline arrow
            Column.bindSheet({ value: this.shown, changeEvent: newValue => { this.shown = newValue; } }, { builder: this.sheetBody.bind(this) }, {
                title: () => {
                    Text('inline title');
                }
            });
        }, Column);
        // 1 title as an inline arrow
        Column.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            // 2 title as a builder reference
            Column.create();
            // 2 title as a builder reference
            Column.bindSheet({ value: this.shown, changeEvent: newValue => { this.shown = newValue; } }, { builder: this.sheetBody.bind(this) }, { title: this.sheetTitle.bind(this) });
        }, Column);
        // 2 title as a builder reference
        Column.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            // 3 a builder method literally named BindSheetTitleCustomBuilder
            Column.create();
            // 3 a builder method literally named BindSheetTitleCustomBuilder
            Column.bindSheet({ value: this.shown, changeEvent: newValue => { this.shown = newValue; } }, { builder: this.sheetBody.bind(this) }, { title: this.BindSheetTitleCustomBuilder.bind(this) });
        }, Column);
        // 3 a builder method literally named BindSheetTitleCustomBuilder
        Column.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "BindSheetTitleProbe";
    }
}
registerNamedRoute(() => new BindSheetTitleProbe(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
