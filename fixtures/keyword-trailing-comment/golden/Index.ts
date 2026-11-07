if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface KeywordTrailing_Params {
    x?: boolean;
    y?: boolean;
}
class KeywordTrailing extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__x = new ObservedPropertySimplePU(true, this, "x");
        this.__y = new ObservedPropertySimplePU(true, this, "y");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: KeywordTrailing_Params) {
        if (params.x !== undefined) {
            this.x = params.x;
        }
        if (params.y !== undefined) {
            this.y = params.y;
        }
    }
    updateStateVars(params: KeywordTrailing_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__x.purgeDependencyOnElmtId(rmElmtId);
        this.__y.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__x.aboutToBeDeleted();
        this.__y.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __x: ObservedPropertySimplePU<boolean>;
    get x() {
        return this.__x.get();
    }
    set x(newValue: boolean) {
        this.__x.set(newValue);
    }
    private __y: ObservedPropertySimplePU<boolean>;
    get y() {
        return this.__y.get();
    }
    set y(newValue: boolean) {
        this.__y.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            if (this.x) {
                this.ifElseBranchUpdateFunction(0, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create('a');
                    }, Text);
                    Text.pop();
                });
            }
            else // ELSE_TRAILING
             {
                this.ifElseBranchUpdateFunction(1, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create('b');
                    }, Text);
                    Text.pop();
                });
            }
        }, If);
        If.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            if (this.y) {
                this.ifElseBranchUpdateFunction(0, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create('c');
                    }, Text);
                    Text.pop();
                });
            } // BRACE_BEFORE_ELSE
            else {
                this.ifElseBranchUpdateFunction(1, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create('d');
                    }, Text);
                    Text.pop();
                });
            }
        }, If);
        If.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            if (this.x) {
                this.ifElseBranchUpdateFunction(0, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        If.create();
                        if (this.y) {
                            this.ifElseBranchUpdateFunction(0, () => {
                                this.observeComponentCreation2((elmtId, isInitialRender) => {
                                    Text.create('e');
                                }, Text);
                                Text.pop();
                            });
                        }
                        else {
                            this.ifElseBranchUpdateFunction(1, () => {
                            });
                        } // INNER_BRACE
                    }, If);
                    If.pop();
                });
            }
            else {
                this.ifElseBranchUpdateFunction(1, () => {
                });
            }
        }, If);
        If.pop();
        Column.pop();
    } // BUILD_END_TRAILING
    helper(): number {
        return 1; // STATEMENT_TRAILING
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "KeywordTrailing";
    }
}
registerNamedRoute(() => new KeywordTrailing(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
