if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ActionPanel_Params {
    actions?: () => void;
}
interface Index_Params {
    color?: string;
}
import { panelBuilder } from "@normalized:N&&&entry/src/main/ets/common/Panel&";
class Index extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__color = new ObservedPropertySimplePU('#ffffff', this, "color");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: Index_Params) {
        if (params.color !== undefined) {
            this.color = params.color;
        }
    }
    updateStateVars(params: Index_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__color.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__color.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __color: ObservedPropertySimplePU<string>;
    get color() {
        return this.__color.get();
    }
    set color(newValue: string) {
        this.__color.set(newValue);
    }
    TabBarBuilder(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('tab');
        }, Text);
        Text.pop();
    }
    UnitBuilder(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('unit');
        }, Text);
        Text.pop();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Tabs.create();
        }, Tabs);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            TabContent.create(() => {
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    Text.create('first');
                }, Text);
                Text.pop();
            });
            TabContent.tabBar({ builder: this.TabBarBuilder.bind(this) });
        }, TabContent);
        TabContent.pop();
        Tabs.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            TextInput.create({ placeholder: 'value' });
            TextInput.showUnit(this.UnitBuilder.bind(this));
        }, TextInput);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('menu');
            Text.bindMenu({ builder: panelBuilder.bind(this) });
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('value');
            Text.background(this.color);
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
class ActionPanel extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.actions = this.emptyActions;
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ActionPanel_Params) {
        if (params.actions !== undefined) {
            this.actions = params.actions;
        }
    }
    updateStateVars(params: ActionPanel_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __actions;
    emptyActions(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('none');
        }, Text);
        Text.pop();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('panel');
            Text.bindMenu({ builder: this.actions.bind(this) });
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
registerNamedRoute(() => new Index(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
