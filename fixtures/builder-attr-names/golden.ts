if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface Index_Params {
}
class Index extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: Index_Params) {
    }
    updateStateVars(params: Index_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    Preview(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('preview');
        }, Text);
        Text.pop();
    }
    VirtualNode(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('virtual');
        }, Text);
        Text.pop();
    }
    Unit(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('unit');
        }, Text);
        Text.pop();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('drag');
            Text.dragPreview({ builder: () => {
                    this.Preview.call(this);
                } });
            Text.accessibilityVirtualNode({ builder: () => {
                    this.VirtualNode.call(this);
                } });
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            TextInput.create({ placeholder: 'value' });
            TextInput.showUnit(() => {
                this.Unit.call(this);
            });
        }, TextInput);
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "Index";
    }
}
registerNamedRoute(() => new Index(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
