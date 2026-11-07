if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface BarAfter_Params {
    n?: number;
}
import { StructFirst, ClassAfter } from "@normalized:N&&&entry/src/main/ets/shapes/struct-first&";
import { OnlyClass } from "@normalized:N&&&entry/src/main/ets/shapes/class-only&";
import { FuncThenStruct, leading } from "@normalized:N&&&entry/src/main/ets/shapes/func-first&";
// PRO_INDEX
// PRO_INDEX_2
export class OptionsBefore {
    // CLS_BODY
    value: number = 1;
}
export class BarAfter extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__n = new ObservedPropertySimplePU(0, this, "n");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: BarAfter_Params) {
        if (params.n !== undefined) {
            this.n = params.n;
        }
    }
    updateStateVars(params: BarAfter_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__n.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__n.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    // STRUCT_BODY
    private __n: ObservedPropertySimplePU<number>;
    get n() {
        return this.__n.get();
    }
    set n(newValue: number) {
        this.__n.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new StructFirst(this, {}, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 21, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {};
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {});
                }
            }, { name: "StructFirst" });
        }
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new FuncThenStruct(this, {}, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 22, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {};
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {});
                }
            }, { name: "FuncThenStruct" });
        }
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${new OnlyClass().run() + leading() + new ClassAfter().v}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "BarAfter";
    }
}
registerNamedRoute(() => new BarAfter(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
