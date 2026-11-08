if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface Child_Params {
    icon1?: Resource;
    isBackPress?: boolean;
    name?: string;
}
interface Index_Params {
    show?: boolean;
}
class Index extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__show = new ObservedPropertySimplePU(true, this, "show");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: Index_Params) {
        if (params.show !== undefined) {
            this.show = params.show;
        }
    }
    updateStateVars(params: Index_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__show.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__show.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __show: ObservedPropertySimplePU<boolean>;
    get show() {
        return this.__show.get();
    }
    set show(newValue: boolean) {
        this.__show.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            if (this.show) {
                this.ifElseBranchUpdateFunction(0, () => {
                    {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            if (isInitialRender) {
                                let componentCall = new Child(this, { name: 'hello' }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 14, col: 9 });
                                ViewPU.create(componentCall);
                                let paramsLambda = () => {
                                    return {
                                        name: 'hello'
                                    };
                                };
                                componentCall.paramsGenerator_ = paramsLambda;
                            }
                            else {
                                this.updateStateVarsOfChildByElmtId(elmtId, {
                                    name: 'hello'
                                });
                            }
                        }, { name: "Child" });
                    }
                });
            }
            else {
                this.ifElseBranchUpdateFunction(1, () => {
                });
            }
        }, If);
        If.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "Index";
    }
}
class Child extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__icon1 = new SynchedPropertyObjectOneWayPU(params.icon1, this, "icon1");
        this.__isBackPress = new SynchedPropertySimpleOneWayPU(params.isBackPress, this, "isBackPress");
        this.__name = new SynchedPropertySimpleOneWayPU(params.name, this, "name");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: Child_Params) {
        if (params.name === undefined) {
            this.__name.set('');
        }
    }
    updateStateVars(params: Child_Params) {
        this.__icon1.reset(params.icon1);
        this.__isBackPress.reset(params.isBackPress);
        this.__name.reset(params.name);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__icon1.purgeDependencyOnElmtId(rmElmtId);
        this.__isBackPress.purgeDependencyOnElmtId(rmElmtId);
        this.__name.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__icon1.aboutToBeDeleted();
        this.__isBackPress.aboutToBeDeleted();
        this.__name.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __icon1?: SynchedPropertySimpleOneWayPU<Resource>;
    get icon1() {
        return this.__icon1.get();
    }
    set icon1(newValue: Resource) {
        this.__icon1.set(newValue);
    }
    private __isBackPress?: SynchedPropertySimpleOneWayPU<boolean>;
    get isBackPress() {
        return this.__isBackPress.get();
    }
    set isBackPress(newValue: boolean) {
        this.__isBackPress.set(newValue);
    }
    private __name: SynchedPropertySimpleOneWayPU<string>;
    get name() {
        return this.__name.get();
    }
    set name(newValue: string) {
        this.__name.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(this.name);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
registerNamedRoute(() => new Index(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
