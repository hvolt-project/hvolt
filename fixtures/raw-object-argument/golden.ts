if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface Index_Params {
    options?: Options;
    count?: number;
    mode?: Mode;
}
// `process_component_class.js` `isNeedGetRawObject`: an object-typed state member handed to a
// method call is read through `ObservedObject.GetRawObject`. A member whose declared type is one
// of `process_component_member.js`'s `simpleTypes` — `string`, `number`, `boolean`, an enum — is
// not, and neither is a read that only takes part in a larger expression.
class Options {
    series: string[] = [];
}
enum Mode {
    Off = 0,
    On = 1
}
class Index extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__options = new ObservedPropertyObjectPU(new Options(), this, "options");
        this.__count = new ObservedPropertySimplePU(0, this, "count");
        this.__mode = new ObservedPropertySimplePU(Mode.Off, this, "mode");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: Index_Params) {
        if (params.options !== undefined) {
            this.options = params.options;
        }
        if (params.count !== undefined) {
            this.count = params.count;
        }
        if (params.mode !== undefined) {
            this.mode = params.mode;
        }
    }
    updateStateVars(params: Index_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__options.purgeDependencyOnElmtId(rmElmtId);
        this.__count.purgeDependencyOnElmtId(rmElmtId);
        this.__mode.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__options.aboutToBeDeleted();
        this.__count.aboutToBeDeleted();
        this.__mode.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __options: ObservedPropertyObjectPU<Options>;
    get options() {
        return this.__options.get();
    }
    set options(newValue: Options) {
        this.__options.set(newValue);
    }
    private __count: ObservedPropertySimplePU<number>;
    get count() {
        return this.__count.get();
    }
    set count(newValue: number) {
        this.__count.set(newValue);
    }
    private __mode: ObservedPropertySimplePU<Mode>;
    get mode() {
        return this.__mode.get();
    }
    set mode(newValue: Mode) {
        this.__mode.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${Object.keys(ObservedObject.GetRawObject(this.options)).length}`);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${Math.max(this.count, 0)}`);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${Math.min(this.mode)}`);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.options.series.length}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "Index";
    }
}
registerNamedRoute(() => new Index(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
