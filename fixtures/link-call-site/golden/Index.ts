if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ObjChild_Params {
    item?: Model;
}
interface PlainChild_Params {
    linked?: number;
}
interface LinkDialog_Params {
    controller?: CustomDialogController;
    linked?: number;
    copied?: boolean;
}
interface ReusableChild_Params {
    linked?: number;
}
interface LinkChild_Params {
    linked?: number;
    copied?: boolean;
}
interface Index_Params {
    count?: number;
    flag?: boolean;
    model?: Model;
    dialog?: CustomDialogController;
    plainDialog?: CustomDialogController;
}
@Observed
class Model {
    value: number = 0;
}
class Index extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__count = new ObservedPropertySimplePU(0, this, "count");
        this.__flag = new ObservedPropertySimplePU(false, this, "flag");
        this.__model = new ObservedPropertyObjectPU(new Model(), this, "model");
        this.dialog = new CustomDialogController({
            builder: () => {
                let jsDialog = new LinkDialog(this, { linked: this.__count, copied: this.flag }, undefined, -1, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 14, col: 14 });
                jsDialog.setController(this.dialog);
                ViewPU.create(jsDialog);
                let paramsLambda = () => {
                    return {
                        linked: this.__count,
                        copied: this.flag
                    };
                };
                jsDialog.paramsGenerator_ = paramsLambda;
            }
        }, this);
        this.plainDialog = new CustomDialogController({
            builder: PlainChild({ linked: this.__count })
        }, this);
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: Index_Params) {
        if (params.count !== undefined) {
            this.count = params.count;
        }
        if (params.flag !== undefined) {
            this.flag = params.flag;
        }
        if (params.model !== undefined) {
            this.model = params.model;
        }
        if (params.dialog !== undefined) {
            this.dialog = params.dialog;
        }
        if (params.plainDialog !== undefined) {
            this.plainDialog = params.plainDialog;
        }
    }
    updateStateVars(params: Index_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__count.purgeDependencyOnElmtId(rmElmtId);
        this.__flag.purgeDependencyOnElmtId(rmElmtId);
        this.__model.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__count.aboutToBeDeleted();
        this.__flag.aboutToBeDeleted();
        this.__model.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __count: ObservedPropertySimplePU<number>;
    get count() {
        return this.__count.get();
    }
    set count(newValue: number) {
        this.__count.set(newValue);
    }
    private __flag: ObservedPropertySimplePU<boolean>;
    get flag() {
        return this.__flag.get();
    }
    set flag(newValue: boolean) {
        this.__flag.set(newValue);
    }
    private __model: ObservedPropertyObjectPU<Model>;
    get model() {
        return this.__model.get();
    }
    set model(newValue: Model) {
        this.__model.set(newValue);
    }
    private dialog: CustomDialogController;
    private plainDialog: CustomDialogController;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new LinkChild(this, { linked: this.__count, copied: this.flag }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 22, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            linked: this.count,
                            copied: this.flag
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {
                        copied: this.flag
                    });
                }
            }, { name: "LinkChild" });
        }
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            __Recycle__.create();
        }, __Recycle__);
        {
            this.observeRecycleComponentCreation("ReusableChild", (elmtId, isInitialRender, recycleNode = null) => {
                ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
                if (isInitialRender) {
                    let componentCall = recycleNode ? recycleNode : new ReusableChild(this, { linked: this.__count }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 23, col: 7 });
                    ViewPU.createRecycle(componentCall, recycleNode !== null, "ReusableChild", () => {
                        if (recycleNode && typeof recycleNode.aboutToReuseInternal === "function") {
                            recycleNode.aboutToReuseInternal();
                        }
                        else {
                            if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === "function") {
                                recycleNode.aboutToReuse({ linked: this.count });
                            }
                            recycleNode.rerender();
                        }
                    });
                    let paramsLambda = () => {
                        return {
                            linked: this.count
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {});
                }
                ViewStackProcessor.StopGetAccessRecording();
            });
        }
        __Recycle__.pop();
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new ObjChild(this, { item: this.model }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 24, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            item: this.model
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {
                        item: this.model
                    });
                }
            }, { name: "ObjChild" });
        }
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new ParamChild(this, { value: 'p' }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 25, col: 7 });
                    ViewV2.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            value: 'p'
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {
                        value: 'p'
                    });
                }
            }, { name: "ParamChild" });
        }
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "Index";
    }
}
class LinkChild extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__linked = new SynchedPropertySimpleTwoWayPU(params.linked, this, "linked");
        this.__copied = new SynchedPropertySimpleOneWayPU(params.copied, this, "copied");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: LinkChild_Params) {
        if (params.copied === undefined) {
            this.__copied.set(false);
        }
    }
    updateStateVars(params: LinkChild_Params) {
        this.__copied.reset(params.copied);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__linked.purgeDependencyOnElmtId(rmElmtId);
        this.__copied.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__linked.aboutToBeDeleted();
        this.__copied.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __linked: SynchedPropertySimpleTwoWayPU<number>;
    get linked() {
        return this.__linked.get();
    }
    set linked(newValue: number) {
        this.__linked.set(newValue);
    }
    private __copied: SynchedPropertySimpleOneWayPU<boolean>;
    get copied() {
        return this.__copied.get();
    }
    set copied(newValue: boolean) {
        this.__copied.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.linked} ${this.copied}`);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class ReusableChild extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__linked = new SynchedPropertySimpleTwoWayPU(params.linked, this, "linked");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ReusableChild_Params) {
    }
    updateStateVars(params: ReusableChild_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__linked.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__linked.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    updateRecycleElmtId(oldElmtId, newElmtId) {
        this.__linked.updateElmtId(oldElmtId, newElmtId);
    }
    private __linked: SynchedPropertySimpleTwoWayPU<number>;
    get linked() {
        return this.__linked.get();
    }
    set linked(newValue: number) {
        this.__linked.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.linked}`);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class LinkDialog extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.controller = undefined;
        this.__linked = new SynchedPropertySimpleTwoWayPU(params.linked, this, "linked");
        this.__copied = new SynchedPropertySimpleOneWayPU(params.copied, this, "copied");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: LinkDialog_Params) {
        if (params.controller !== undefined) {
            this.controller = params.controller;
        }
        if (params.copied === undefined) {
            this.__copied.set(false);
        }
    }
    updateStateVars(params: LinkDialog_Params) {
        this.__copied.reset(params.copied);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__linked.purgeDependencyOnElmtId(rmElmtId);
        this.__copied.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__linked.aboutToBeDeleted();
        this.__copied.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private controller?: CustomDialogController;
    setController(ctr: CustomDialogController) {
        this.controller = ctr;
    }
    private __linked: SynchedPropertySimpleTwoWayPU<number>;
    get linked() {
        return this.__linked.get();
    }
    set linked(newValue: number) {
        this.__linked.set(newValue);
    }
    private __copied: SynchedPropertySimpleOneWayPU<boolean>;
    get copied() {
        return this.__copied.get();
    }
    set copied(newValue: boolean) {
        this.__copied.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.linked} ${this.copied}`);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class PlainChild extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__linked = new SynchedPropertySimpleTwoWayPU(params.linked, this, "linked");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: PlainChild_Params) {
    }
    updateStateVars(params: PlainChild_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__linked.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__linked.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __linked: SynchedPropertySimpleTwoWayPU<number>;
    get linked() {
        return this.__linked.get();
    }
    set linked(newValue: number) {
        this.__linked.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.linked}`);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class ObjChild extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__item = new SynchedPropertyNesedObjectPU(params.item, this, "item");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ObjChild_Params) {
        this.__item.set(params.item);
    }
    updateStateVars(params: ObjChild_Params) {
        this.__item.set(params.item);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__item.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__item.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __item: SynchedPropertyNesedObjectPU<Model>;
    get item() {
        return this.__item.get();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.item.value}`);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class ParamChild extends ViewV2 {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda, extraInfo) {
        super(parent, elmtId, extraInfo);
        this.initParam("value", (params && "value" in params) ? params.value : '');
        this.finalizeConstruction();
    }
    public resetStateVarsOnReuse(params: Object): void {
        this.resetParam("value", (params && "value" in params) ? params.value : '');
    }
    @Param
    readonly value: string;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(this.value);
        }, Text);
        Text.pop();
    }
    public updateStateVars(params) {
        if (params === undefined) {
            return;
        }
        if ("value" in params) {
            this.updateParam("value", params.value);
        }
    }
    rerender() {
        this.updateDirtyElements();
    }
}
registerNamedRoute(() => new Index(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
