if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface Index_Params {
	标题?: string;
}
interface UnicodeComments_Params {
	计数?: number;
}
class UnicodeComments extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__计数 = new ObservedPropertySimplePU(0, this, "计数");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: UnicodeComments_Params) {
        if (params.计数 !== undefined) {
            this.计数 = params.计数;
        }
    }
    updateStateVars(params: UnicodeComments_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__计数.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__计数.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __计数: ObservedPropertySimplePU<number>;
    get 计数() {
        return this.__计数.get();
    }
    set 计数(newValue: number) {
        this.__计数.set(newValue);
    }
    // 方法前的注释：用于验证注释不会被改写
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
            Column.width('100%');
        }, Column);
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // 行注释里的分号; 与逗号, 与冒号: 都不应影响结构
                Text.create(`当前值：${this.计数}，单位“个”`);
                // 行注释里的分号; 与逗号, 与冒号: 都不应影响结构
                Text.fontSize(16);
            }, Text);
            // 行注释里的分号; 与逗号, 与冒号: 都不应影响结构
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                Button.createWithLabel('点一下加一');
                Button.onClick(() => {
          // 自增
          this.计数 += 1
        });
            }, Button);
            Button.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class Index extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__标题 = new ObservedPropertySimplePU('中文标题', this, '标题');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: Index_Params) {
		if (params.标题 !== undefined) {
			this.标题 = params.标题;
		}
	}
	updateStateVars(params: Index_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__标题.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__标题.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __标题: ObservedPropertySimplePU<string>;
	get 标题() {
		return this.__标题.get();
	}
	set 标题(newValue: string) {
		this.__标题.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create({ space: 8 });
			Column.padding(12);
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.标题);
			Text.fontSize(20);
		}, Text);
		Text.pop();
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new UnicodeComments(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 31,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
			}, { name: 'UnicodeComments' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'Index';
	}
}
registerNamedRoute(() => new Index(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
