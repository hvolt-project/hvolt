if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface DoubleExclaim_Params {
    value?: number;
    isShowMenu?: boolean;
    cond?: boolean;
}
class DoubleExclaim extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__value = new ObservedPropertySimplePU(1, this, "value");
        this.__isShowMenu = new ObservedPropertySimplePU(false, this, "isShowMenu");
        this.__cond = new ObservedPropertySimplePU(true, this, "cond");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: DoubleExclaim_Params) {
        if (params.value !== undefined) {
            this.value = params.value;
        }
        if (params.isShowMenu !== undefined) {
            this.isShowMenu = params.isShowMenu;
        }
        if (params.cond !== undefined) {
            this.cond = params.cond;
        }
    }
    updateStateVars(params: DoubleExclaim_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__value.purgeDependencyOnElmtId(rmElmtId);
        this.__isShowMenu.purgeDependencyOnElmtId(rmElmtId);
        this.__cond.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__value.aboutToBeDeleted();
        this.__isShowMenu.aboutToBeDeleted();
        this.__cond.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __value: ObservedPropertySimplePU<number>;
    get value() {
        return this.__value.get();
    }
    set value(newValue: number) {
        this.__value.set(newValue);
    }
    private __isShowMenu: ObservedPropertySimplePU<boolean>;
    get isShowMenu() {
        return this.__isShowMenu.get();
    }
    set isShowMenu(newValue: boolean) {
        this.__isShowMenu.set(newValue);
    }
    private __cond: ObservedPropertySimplePU<boolean>;
    get cond() {
        return this.__cond.get();
    }
    set cond(newValue: boolean) {
        this.__cond.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Slider.create({ value: this.value!! as number, min: 0, max: 10 });
            Slider.showSteps(true);
        }, Slider);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Slider.create({ value: this.value, $value: newValue => { this.value = newValue; } });
        }, Slider);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.value}`);
            Text.bindPopup({ value: this.isShowMenu, $value: newValue => { this.isShowMenu = newValue; } }, { message: 'shown' });
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('cond');
            Text.bindPopup(this.cond ? this.isShowMenu!! : null, { message: 'cond' });
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "DoubleExclaim";
    }
}
registerNamedRoute(() => new DoubleExclaim(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
