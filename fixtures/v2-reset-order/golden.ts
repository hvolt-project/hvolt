if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
class V2ResetOrder extends ViewV2 {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
        super(parent, elmtId, extraInfo);
        this.before = 1;
        this.after = 2;
        this.finalizeConstruction();
    }
    public resetStateVarsOnReuse(params: Object): void {
        this.before = 1;
        this.resetComputed("doubled");
        this.after = 2;
        this.resetMonitorsOnReuse();
    }
    @Local
    before: number;
    @Computed
    get doubled(): number {
        return this.before * 2;
    }
    @Local
    after: number;
    @Monitor('before')
    onBefore(mon: IMonitor): void {
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.doubled}${this.after}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "V2ResetOrder";
    }
}
registerNamedRoute(() => new V2ResetOrder(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
