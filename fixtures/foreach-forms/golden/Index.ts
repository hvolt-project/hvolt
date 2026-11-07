if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ForEachForms_Params {
    rows?: RowItem[];
}
class RowItem {
    id: number = 0;
    name: string = '';
}
const HEADS: string[] = ['a', 'b'];
class ForEachForms extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__rows = new ObservedPropertyObjectPU([], this, "rows");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ForEachForms_Params) {
        if (params.rows !== undefined) {
            this.rows = params.rows;
        }
    }
    updateStateVars(params: ForEachForms_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__rows.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__rows.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __rows: ObservedPropertyObjectPU<RowItem[]>;
    get rows() {
        return this.__rows.get();
    }
    set rows(newValue: RowItem[]) {
        this.__rows.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            ForEach.create();
            const forEachItemGenFunction = _item => {
                const head = _item;
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    Text.create(head);
                }, Text);
                Text.pop();
            };
            this.forEachUpdateFunction(elmtId, HEADS, forEachItemGenFunction);
        }, ForEach);
        ForEach.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            ForEach.create();
            const forEachItemGenFunction = (_item, index: number) => {
                const row = _item;
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    Text.create(`${index}${row.name}`);
                }, Text);
                Text.pop();
            };
            this.forEachUpdateFunction(elmtId, this.rows, forEachItemGenFunction, (row: RowItem) => row.name, true, false);
        }, ForEach);
        ForEach.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            ForEach.create();
            const forEachItemGenFunction = _item => {
                const row = _item;
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    Text.create(row.name);
                }, Text);
                Text.pop();
            };
            this.forEachUpdateFunction(elmtId, this.rows, forEachItemGenFunction, (row: RowItem, index: number) => `${index}`, false, true);
        }, ForEach);
        ForEach.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            ForEach.create();
            const forEachItemGenFunction = (_item, index: number) => {
                const row = _item;
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    Text.create(`${index}${row.name}`);
                }, Text);
                Text.pop();
            };
            this.forEachUpdateFunction(elmtId, this.rows, forEachItemGenFunction, undefined, true, false);
        }, ForEach);
        ForEach.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            ForEach.create();
            const forEachItemGenFunction = _item => {
                const head = _item;
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    Row.create();
                }, Row);
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    ForEach.create();
                    const forEachItemGenFunction = _item => {
                        const inner = _item;
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create(inner.name);
                        }, Text);
                        Text.pop();
                    };
                    this.forEachUpdateFunction(elmtId, this.rows, forEachItemGenFunction);
                }, ForEach);
                ForEach.pop();
                Row.pop();
            };
            this.forEachUpdateFunction(elmtId, HEADS, forEachItemGenFunction, (head: string) => head, false, false);
        }, ForEach);
        ForEach.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "ForEachForms";
    }
}
registerNamedRoute(() => new ForEachForms(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
