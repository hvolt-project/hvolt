if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface EnumInitializers_Params {
    mode?: Mode;
    label?: string;
}
// Enum members without an initializer take one from the previous member: the first is 0, and
// a member after an explicit one is that value + 1. A string member is never filled.
enum Mode {
    Off = 0,
    On = 1
}
enum Level {
    Low = 3,
    Mid = 4,
    High = 5
}
enum Mixed {
    A = 0,
    B = 5,
    C = 6
}
enum Named {
    X = "x",
    Y = "y"
}
class EnumInitializers extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__mode = new ObservedPropertySimplePU(Mode.On, this, "mode");
        this.__label = new ObservedPropertySimplePU(`${Level.High}/${Mixed.C}/${Named.Y}`, this, "label");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: EnumInitializers_Params) {
        if (params.mode !== undefined) {
            this.mode = params.mode;
        }
        if (params.label !== undefined) {
            this.label = params.label;
        }
    }
    updateStateVars(params: EnumInitializers_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__mode.purgeDependencyOnElmtId(rmElmtId);
        this.__label.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__mode.aboutToBeDeleted();
        this.__label.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __mode: ObservedPropertySimplePU<Mode>;
    get mode() {
        return this.__mode.get();
    }
    set mode(newValue: Mode) {
        this.__mode.set(newValue);
    }
    private __label: ObservedPropertySimplePU<string>;
    get label() {
        return this.__label.get();
    }
    set label(newValue: string) {
        this.__label.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.mode} ${this.label}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "EnumInitializers";
    }
}
registerNamedRoute(() => new EnumInitializers(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
