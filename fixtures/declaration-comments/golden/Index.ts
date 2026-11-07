if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface DeclIndex_Params {
}
interface DeclHolder_Params {
    count?: number;
    title?: string;
    label?: string;
}
// BUILDER_DECL_COMMENT
export function declBuilder(label: string, parent = null) {
    const __label__ = label;
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, label = __label__) => {
        Text.create(label);
    }, Text);
    Text.pop();
}
// FUNCTION_DECL_COMMENT
function plainHelper(value: number): number {
    return value + 1;
}
// INTERFACE_FIELD_COMMENT
interface DeclShape {
    // INTERFACE_MEMBER_COMMENT
    name: string;
    size: number;
}
// ENUM_MEMBER_COMMENT
enum DeclKind {
    // ENUM_FIRST
    First = 0,
    Second = 1
}
export class DeclHolder extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__count = new ObservedPropertySimplePU(0
        // PROP_FIELD_COMMENT
        , this, "count");
        this.__title = new SynchedPropertySimpleOneWayPU(params.title, this, "title");
        this.label = 'x';
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: DeclHolder_Params) {
        if (params.count !== undefined) {
            this.count = params.count;
        }
        if (params.title === undefined) {
            this.__title.set(''
            // PLAIN_FIELD_COMMENT
            );
        }
        if (params.label !== undefined) {
            this.label = params.label;
        }
    }
    updateStateVars(params: DeclHolder_Params) {
        this.__title.reset(params.title);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__count.purgeDependencyOnElmtId(rmElmtId);
        this.__title.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__count.aboutToBeDeleted();
        this.__title.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    // STATE_FIELD_COMMENT
    private __count: ObservedPropertySimplePU<number>;
    get count() {
        return this.__count.get();
    }
    set count(newValue: number) {
        this.__count.set(newValue);
    }
    // PROP_FIELD_COMMENT
    private __title: SynchedPropertySimpleOneWayPU<string>;
    get title() {
        return this.__title.get();
    }
    set title(newValue: string) {
        this.__title.set(newValue);
    }
    // PLAIN_FIELD_COMMENT
    private label: string;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(this.title);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(plainHelper(this.count).toString());
        }, Text);
        Text.pop();
        this.item.bind(this)('a');
        Column.pop();
    }
    // METHOD_DECL_COMMENT
    item(name: string, parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(name);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class DeclIndex extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: DeclIndex_Params) {
    }
    updateStateVars(params: DeclIndex_Params) {
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
        declBuilder.bind(this)('d');
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new DeclHolder(this, { title: 't' }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 57, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            title: 't'
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {
                        title: 't'
                    });
                }
            }, { name: "DeclHolder" });
        }
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "DeclIndex";
    }
}
registerNamedRoute(() => new DeclIndex(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
