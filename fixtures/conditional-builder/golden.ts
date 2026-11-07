if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface Index_Params {
    cond?: boolean;
    actions?: () => void;
}
function MyMenu(parent = null) {
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
        Menu.create();
    }, Menu);
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
        MenuItem.create({ content: 'mine' });
    }, MenuItem);
    MenuItem.pop();
    Menu.pop();
}
function OtherMenu(parent = null) {
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
        Menu.create();
    }, Menu);
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
        MenuItem.create({ content: 'other' });
    }, MenuItem);
    MenuItem.pop();
    Menu.pop();
}
function ArgMenu(label: string, parent = null) {
    const __label__ = label;
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, label = __label__) => {
        Menu.create();
    }, Menu);
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, label = __label__) => {
        MenuItem.create({ content: label });
    }, MenuItem);
    MenuItem.pop();
    Menu.pop();
}
function UnitOne(parent = null) {
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
        Text.create('one');
    }, Text);
    Text.pop();
}
class Index extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__cond = new ObservedPropertySimplePU(false, this, "cond");
        this.actions = this.emptyActions;
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: Index_Params) {
        if (params.cond !== undefined) {
            this.cond = params.cond;
        }
        if (params.actions !== undefined) {
            this.actions = params.actions;
        }
    }
    updateStateVars(params: Index_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__cond.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__cond.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __cond: ObservedPropertySimplePU<boolean>;
    get cond() {
        return this.__cond.get();
    }
    set cond(newValue: boolean) {
        this.__cond.set(newValue);
    }
    ThisMenu(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Menu.create();
        }, Menu);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            MenuItem.create({ content: 'this' });
        }, MenuItem);
        MenuItem.pop();
        Menu.pop();
    }
    ThisArg(label: string, parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Menu.create();
        }, Menu);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            MenuItem.create({ content: label });
        }, MenuItem);
        MenuItem.pop();
        Menu.pop();
    }
    UnitTwo(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('two');
        }, Text);
        Text.pop();
    }
    emptyActions(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('none');
        }, Text);
        Text.pop();
    }
    private __actions;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('menu');
            Text.bindMenu(this.cond ? { builder: () => {
                    MyMenu.call(this);
                } } : null);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('context');
            Text.bindContextMenu(this.cond ? { builder: () => {
                    MyMenu.call(this);
                } } : { builder: () => {
                    OtherMenu.call(this);
                } }, ResponseType.RightClick);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('this-menu');
            Text.bindMenu(this.cond ? { builder: () => {
                    this.ThisMenu.call(this);
                } } : null);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('cover');
            Text.bindContentCover(this.cond, this.cond ? { builder: () => {
                    this.ThisMenu.call(this);
                } } : null);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('reversed');
            Text.bindMenu(this.cond ? null : { builder: () => {
                    MyMenu.call(this);
                } });
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('arg-global');
            Text.bindMenu(this.cond ? { builder: () => {
                    ArgMenu.call(this, 'y');
                } } : null);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('arg-this');
            Text.bindMenu(this.cond ? { builder: () => {
                    this.ThisArg.call(this, 'x');
                } } : null);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('nested');
            Text.bindMenu(this.cond ? { builder: () => {
                    MyMenu.call(this);
                } } : this.cond ? OtherMenu() : null);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('ident-this');
            Text.bindMenu(this.cond ? { builder: this.actions.bind(this) } : null);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('ident-global');
            Text.bindMenu(this.cond ? { builder: MyMenu.bind(this) } : null);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('second-arg');
            Text.bindMenu(this.cond, this.cond ? { builder: () => {
                    MyMenu.call(this);
                } } : null);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            TextInput.create({ placeholder: 'unit-global' });
            TextInput.showUnit(this.cond ? () => {
                UnitOne.call(this);
            } : null);
        }, TextInput);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            TextInput.create({ placeholder: 'unit-this' });
            TextInput.showUnit(this.cond ? () => {
                this.UnitTwo.call(this);
            } : null);
        }, TextInput);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('popup-global');
            Text.bindPopup(this.cond, { builder: this.cond ? { builder: () => {
                        MyMenu.call(this);
                    } } : null, onWillDismiss: false });
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('popup-this');
            Text.bindPopup(this.cond, { builder: this.cond ? { builder: () => {
                        this.ThisMenu.call(this);
                    } } : null, onWillDismiss: false });
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('popup-param');
            Text.bindPopup(this.cond, { builder: { builder: this.actions.bind(this) }, onWillDismiss: false });
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('popup-global-ident');
            Text.bindPopup(this.cond, { builder: { builder: MyMenu.bind(this) }, onWillDismiss: false });
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
