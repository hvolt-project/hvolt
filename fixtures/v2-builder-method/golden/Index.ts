if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
class V2Builder extends ViewV2 {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
        super(parent, elmtId, extraInfo);
        this.label = 'l';
        this.finalizeConstruction();
    }
    public resetStateVarsOnReuse(params: Object): void {
        this.label = 'l';
    }
    @Local
    label: string;
    chip(t: string, parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            ForEach.create();
            const forEachItemGenFunction = _item => {
                const item = _item;
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    Text.create(item);
                }, Text);
                Text.pop();
            };
            this.forEachUpdateFunction(elmtId, [t], forEachItemGenFunction);
        }, ForEach);
        ForEach.pop();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.chip.bind(this)(this.label);
        Column.pop();
    }
    tail(): number {
        return 1;
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "V2Builder";
    }
}
registerNamedRoute(() => new V2Builder(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
