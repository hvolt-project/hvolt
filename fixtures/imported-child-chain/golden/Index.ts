if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ImportedChildChain_Params {
    selected?: boolean;
}
import { Card } from "@normalized:N&&&entry/src/main/ets/common/Card&";
class ImportedChildChain extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__selected = new ObservedPropertySimplePU(false, this, "selected");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ImportedChildChain_Params) {
        if (params.selected !== undefined) {
            this.selected = params.selected;
        }
    }
    updateStateVars(params: ImportedChildChain_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__selected.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__selected.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __selected: ObservedPropertySimplePU<boolean>;
    get selected() {
        return this.__selected.get();
    }
    set selected(newValue: boolean) {
        this.__selected.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            __Common__.create();
            __Common__.transition(TransitionEffect.OPACITY);
            __Common__.width('100%');
        }, __Common__);
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new Card(this, { title: 'chained' }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 16, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            title: 'chained'
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {
                        title: 'chained'
                    });
                }
            }, { name: "Card" });
        }
        __Common__.pop();
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new Card(this, { title: 'plain' }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 20, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            title: 'plain'
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {
                        title: 'plain'
                    });
                }
            }, { name: "Card" });
        }
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "ImportedChildChain";
    }
}
registerNamedRoute(() => new ImportedChildChain(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
