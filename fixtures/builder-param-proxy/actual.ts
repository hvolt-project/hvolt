if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface BuilderParamProxyProbe_Params {
	shown?: boolean;
	pageTitle?: string;
	shared?: Cfg;
}
// Which argument shapes the reference rewrites when a builder is called, and which it forwards
// as written. The first probe wrote object literals into a `@BuilderParam` field, which the
// ArkTS checker rejects (`Type 'Cfg' provides no match for the signature '(): void'`), so each
// shape sits at a call site whose declared parameter type accepts it.

class Cfg {
  a: number = 1
  b: Cfg | null = null
  run: () => void = () => {
  }
}
const SHARED_CFG: Cfg = {
	a: 1,
	b: null,
	run: () => {}
};
function globalContent(cfg: Cfg, parent = null) {
	const __cfg__ = cfg;
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, cfg = __cfg__) => {
		Column.create();
	}, Column);
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, cfg = __cfg__) => {
		Text.create(`${cfg.a}`);
	}, Text);
	Text.pop();
	Column.pop();
}
class Cfg2 {
	title: string = '';
}
function globalTitleObj(cfg: Cfg2, parent = null) {
	const __cfg__ = cfg;
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, cfg = __cfg__) => {
		Text.create(cfg.title);
	}, Text);
	Text.pop();
}
function globalTitle(text: string, parent = null) {
	const __text__ = text;
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, text = __text__) => {
		Text.create(text);
	}, Text);
	Text.pop();
}
function globalCount(n: number, parent = null) {
	const __n__ = n;
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender, n = __n__) => {
		Text.create(`${n}`);
	}, Text);
	Text.pop();
}
class BuilderParamProxyProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__shown = new ObservedPropertySimplePU(false, this, "shown");
        this.__pageTitle = new ObservedPropertySimplePU('probe', this, "pageTitle");
        this.shared = SHARED_CFG;
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: BuilderParamProxyProbe_Params) {
        if (params.shown !== undefined) {
            this.shown = params.shown;
        }
        if (params.pageTitle !== undefined) {
            this.pageTitle = params.pageTitle;
        }
        if (params.shared !== undefined) {
            this.shared = params.shared;
        }
    }
    updateStateVars(params: BuilderParamProxyProbe_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__shown.purgeDependencyOnElmtId(rmElmtId);
        this.__pageTitle.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__shown.aboutToBeDeleted();
        this.__pageTitle.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __shown: ObservedPropertySimplePU<boolean>;
    get shown() {
        return this.__shown.get();
    }
    set shown(newValue: boolean) {
        this.__shown.set(newValue);
    }
    private __pageTitle: ObservedPropertySimplePU<string>;
    get pageTitle() {
        return this.__pageTitle.get();
    }
    set pageTitle(newValue: string) {
        this.__pageTitle.set(newValue);
    }
    private shared: Cfg;
    memberContent(cfg: Cfg, parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${cfg.a}`);
        }, Text);
        Text.pop();
    }
    memberSheet(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('sheet');
        }, Text);
        Text.pop();
    }
    sheetTitle(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create('title');
        }, Text);
        Text.pop();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            // 1 a lone object literal
            globalContent.bind(this)(makeBuilderParameterProxy("globalContent", { a: () => 1, b: () => null, run: () => () => {
      } }));
            // 2 an object literal carrying a nested object and an arrow
            globalContent.bind(this)(makeBuilderParameterProxy("globalContent", { a: () => 1, b: () => ({ a: 2, b: null, run: () => {
      } }), run: () => () => {
      } }));
            // 3 a declared object constant
            globalContent.bind(this)(SHARED_CFG);
            // 4 a reference rather than a call
            globalContent.bind(this)(this.shared);
            // 5 a member @Builder call with an object literal
this.memberContent.bind(this)(makeBuilderParameterProxy("memberContent", { a: () => 4, b: () => null, run: () => () => {
      } }))
            // 6 a literal argument (control)
            globalCount.bind(this)(2);
            // 7 a @State string read as a builder argument
            globalTitle.bind(this)(this.pageTitle);
            // 8 a @State string read inside the proxy object
            globalTitleObj.bind(this)(makeBuilderParameterProxy("globalTitleObj", { title: () => (this["__pageTitle"] ? this["__pageTitle"] : this["pageTitle"]) }));
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // a CustomBuilder argument that is a reference (control)
                Column.create();
                // a CustomBuilder argument that is a reference (control)
                Column.bindSheet({ value: this.shown, changeEvent: newValue => { this.shown = newValue; } }, { builder: this.memberSheet.bind(this) });
            }, Column);
            // a CustomBuilder argument that is a reference (control)
            Column.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // an inline arrow where a CustomBuilder is expected
                Column.create();
                // an inline arrow where a CustomBuilder is expected
                Column.bindSheet({ value: this.shown, changeEvent: newValue => { this.shown = newValue; } }, () => {
          this.memberSheet()
        });
            }, Column);
            // an inline arrow where a CustomBuilder is expected
            Column.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // a builder used as the sheet title
                Column.create();
                // a builder used as the sheet title
                Column.bindSheet({ value: this.shown, changeEvent: newValue => { this.shown = newValue; } }, { builder: this.memberSheet.bind(this) }, { title: this.sheetTitle.bind(this) });
            }, Column);
            // a builder used as the sheet title
            Column.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "BuilderParamProxyProbe";
    }
}
registerNamedRoute(() => new BuilderParamProxyProbe(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
