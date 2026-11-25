if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
@ObservedV2
class Model {
    @Trace
    size: number = 1;
}
class Batch5Probe extends ViewV2 {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
        super(parent, elmtId, extraInfo);
        this.model = new Model();
        this.finalizeConstruction();
    }
    public resetStateVarsOnReuse(params: Object): void {
        this.model = new Model();
        this.resetMonitorsOnReuse();
    }
    @Local
    model: Model;
    @Monitor("model.size")
    onSize() {
        return 0;
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.model.size}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "Batch5Probe";
    }
}
registerNamedRoute(() => new Batch5Probe(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
