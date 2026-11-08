if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface PropertyAccessThis_Params {
    flag?: boolean;
    primary?: string;
    secondary?: string;
    rows?: RowData[];
}
interface ChildBox_Params {
    header?: () => void;
    label?: string;
}
function FallbackHeader(parent = null) {
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
        Text.create('');
    }, Text);
    Text.pop();
}
class ChildBox extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.header = FallbackHeader;
        this.label = '';
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ChildBox_Params) {
        if (params.header !== undefined) {
            this.header = params.header;
        }
        if (params.label !== undefined) {
            this.label = params.label;
        }
    }
    updateStateVars(params: ChildBox_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __header;
    private label: string;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.header.bind(this)();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(this.label);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
interface RowData {
    name: string;
}
class PropertyAccessThis extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__flag = new ObservedPropertySimplePU(true, this, "flag");
        this.__primary = new ObservedPropertySimplePU('#ff0000', this, "primary");
        this.__secondary = new ObservedPropertySimplePU('#00ff00', this, "secondary");
        this.__rows = new ObservedPropertyObjectPU([{ name: 'a' }, { name: 'b' }], this, "rows");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: PropertyAccessThis_Params) {
        if (params.flag !== undefined) {
            this.flag = params.flag;
        }
        if (params.primary !== undefined) {
            this.primary = params.primary;
        }
        if (params.secondary !== undefined) {
            this.secondary = params.secondary;
        }
        if (params.rows !== undefined) {
            this.rows = params.rows;
        }
    }
    updateStateVars(params: PropertyAccessThis_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__flag.purgeDependencyOnElmtId(rmElmtId);
        this.__primary.purgeDependencyOnElmtId(rmElmtId);
        this.__secondary.purgeDependencyOnElmtId(rmElmtId);
        this.__rows.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__flag.aboutToBeDeleted();
        this.__primary.aboutToBeDeleted();
        this.__secondary.aboutToBeDeleted();
        this.__rows.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __flag: ObservedPropertySimplePU<boolean>;
    get flag() {
        return this.__flag.get();
    }
    set flag(newValue: boolean) {
        this.__flag.set(newValue);
    }
    private __primary: ObservedPropertySimplePU<string>;
    get primary() {
        return this.__primary.get();
    }
    set primary(newValue: string) {
        this.__primary.set(newValue);
    }
    private __secondary: ObservedPropertySimplePU<string>;
    get secondary() {
        return this.__secondary.get();
    }
    set secondary(newValue: string) {
        this.__secondary.set(newValue);
    }
    private __rows: ObservedPropertyObjectPU<RowData[]>;
    get rows() {
        return this.__rows.get();
    }
    set rows(newValue: RowData[]) {
        this.__rows.set(newValue);
    }
    color(): string {
        return this.flag ? this.primary : this.secondary;
    }
    handler(): void {
        this.flag = !this.flag;
    }
    work(): void {
        this.flag = true;
    }
    headerB(text: string, parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(text);
            Text.fontSize(12);
        }, Text);
        Text.pop();
    }
    footerB(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('footer');
        }, Text);
        Text.pop();
    }
    rowFooter(text: string, parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(text);
            Text.fontSize(10);
        }, Text);
        Text.pop();
    }
    refreshB(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('refreshing');
        }, Text);
        Text.pop();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('a');
            Text.fontColor(this.color());
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('b');
            Text.onClick(this.handler);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('c');
            Text.onClick(() => {
                this.work();
            });
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('d');
            Text.onClick(this.work);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('e');
            Text.fontColor(this.flag ? this.primary : this.secondary);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('f');
            Text.onClick(this.work.bind(this));
        }, Text);
        Text.pop();
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new ChildBox(this, { header: this.footerB, label: 'x' }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 73, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            header: this.footerB,
                            label: 'x'
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {});
                }
            }, { name: "ChildBox" });
        }
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            List.create();
        }, List);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            ForEach.create();
            const forEachItemGenFunction = _item => {
                const row = _item;
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    ListItemGroup.create({ header: this.headerB.bind(this, row.name), footer: this.rowFooter.bind(this, row.name) });
                }, ListItemGroup);
                {
                    const itemCreation = (elmtId, isInitialRender) => {
                        ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
                        ListItem.create(deepRenderFunction, true);
                        if (!isInitialRender) {
                            ListItem.pop();
                        }
                        ViewStackProcessor.StopGetAccessRecording();
                    };
                    const itemCreation2 = (elmtId, isInitialRender) => {
                        ListItem.create(deepRenderFunction, true);
                    };
                    const deepRenderFunction = (elmtId, isInitialRender) => {
                        itemCreation(elmtId, isInitialRender);
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create(row.name);
                        }, Text);
                        Text.pop();
                        ListItem.pop();
                    };
                    this.observeComponentCreation2(itemCreation2, ListItem);
                    ListItem.pop();
                }
                ListItemGroup.pop();
            };
            this.forEachUpdateFunction(elmtId, this.rows, forEachItemGenFunction);
        }, ForEach);
        ForEach.pop();
        List.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Refresh.create({ refreshing: this.flag, builder: this.refreshB.bind(this) });
        }, Refresh);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('body');
        }, Text);
        Text.pop();
        Refresh.pop();
        this.footerB.bind(this)();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "PropertyAccessThis";
    }
}
registerNamedRoute(() => new PropertyAccessThis(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
