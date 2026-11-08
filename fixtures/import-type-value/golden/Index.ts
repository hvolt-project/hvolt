if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ImportTypeValue_Params {
    shape?: Shape;
}
import { Mode } from "@normalized:N&&&entry/src/main/ets/pages/Model&";
import type { Shape } from "@normalized:N&&&entry/src/main/ets/pages/Model&";
// `Shape` is an interface, so it has no runtime export: the import has to be `import type`
// even though a syntactic reading counts the `implements` clause as a value position. `Mode`
// is an enum and stays a value import — the two are split into their own statements.
class Circle implements Shape {
    kind: string = 'circle';
    mode: Mode = Mode.Fast;
}
class ImportTypeValue extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__shape = new ObservedPropertyObjectPU(new Circle(), this, "shape");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ImportTypeValue_Params) {
        if (params.shape !== undefined) {
            this.shape = params.shape;
        }
    }
    updateStateVars(params: ImportTypeValue_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__shape.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__shape.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __shape: ObservedPropertyObjectPU<Shape>;
    get shape() {
        return this.__shape.get();
    }
    set shape(newValue: Shape) {
        this.__shape.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.shape.kind} ${Mode.Slow}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "ImportTypeValue";
    }
}
registerNamedRoute(() => new ImportTypeValue(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
