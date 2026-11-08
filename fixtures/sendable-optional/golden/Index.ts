if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface SendableOptional_Params {
}
// `process_sendable.js::transformOptionalMemberForSendable`: inside a `@Sendable` class a
// property written `name?: T` becomes `name: T | undefined = undefined`, and one that writes
// its own initializer keeps it (`init?: string = 'i'` → `init: string | undefined = 'i'`). A
// property without the question token is untouched.
class Box {
    constructor() {
        "use sendable";
    }
    value: number | undefined = undefined;
    init: string | undefined = 'i';
    plain: boolean = false;
    read(): number {
        return this.plain ? 1 : 0;
    }
}
function make(n: number): number {
    "use sendable";
    return n;
}
class SendableOptional extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: SendableOptional_Params) {
    }
    updateStateVars(params: SendableOptional_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${new Box().read()}${make(1)}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "SendableOptional";
    }
}
registerNamedRoute(() => new SendableOptional(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
