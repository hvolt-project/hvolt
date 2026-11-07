if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface IndentUnit_Params {
    fieldProbe?: number;
}
import { PlainUnit } from "@normalized:N&&&entry/src/main/ets/util/PlainUnit&";
class IndentUnit extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.fieldProbe = 0;
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: IndentUnit_Params) {
        if (params.fieldProbe !== undefined) {
            this.fieldProbe = params.fieldProbe;
        }
    }
    updateStateVars(params: IndentUnit_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    /**
     * FIELD_BLOCK
     */
    private fieldProbe: number;
    /**
     * TWO_SPACE_BLOCK
     */
    private twoSpace(): number {
        /**
         * TWO_SPACE_INNER
         */
        return 1;
    }
    /**
     * FOUR_SPACE_BLOCK
     */
    private fourSpace(): number {
        return 2;
    }
    private templateText(): string {
        return `line1
      TEMPLATE_INDENT
    line3`;
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.twoSpace()}${this.fourSpace()}`);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(this.templateText());
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.fieldProbe}${PlainUnit.value()}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "IndentUnit";
    }
}
registerNamedRoute(() => new IndentUnit(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
