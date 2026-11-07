if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface ShorthandProperty_Params {
	name?: string;
	ident?: string;
	items?: Pair[];
}
interface ReusableRow_Params {
	name?: string;
}
interface ChildRow_Params {
	name?: string;
	tag?: string;
}
interface Pair {
	name: string;
	key: string;
	value: number;
}
interface Nested {
	inner: Pair;
}
function getFn(): string {
	return 'from-fn';
}
class ChildRow extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__name = new SynchedPropertySimpleOneWayPU(params.name, this, 'name');
		this.__tag = new SynchedPropertySimpleOneWayPU(params.tag, this, 'tag');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ChildRow_Params) {
		if (params.name === undefined) {
			this.__name.set('');
		}
		if (params.tag === undefined) {
			this.__tag.set('');
		}
	}
	updateStateVars(params: ChildRow_Params) {
		this.__name.reset(params.name);
		this.__tag.reset(params.tag);
	}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__name.purgeDependencyOnElmtId(rmElmtId);
		this.__tag.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__name.aboutToBeDeleted();
		this.__tag.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __name: SynchedPropertySimpleOneWayPU<string>;
	get name() {
		return this.__name.get();
	}
	set name(newValue: string) {
		this.__name.set(newValue);
	}
	private __tag: SynchedPropertySimpleOneWayPU<string>;
	get tag() {
		return this.__tag.get();
	}
	set tag(newValue: string) {
		this.__tag.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Row.create();
		}, Row);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.name);
		}, Text);
		Text.pop();
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.tag);
		}, Text);
		Text.pop();
		Row.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class ReusableRow extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__name = new SynchedPropertySimpleOneWayPU(params.name, this, 'name');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ReusableRow_Params) {
		if (params.name === undefined) {
			this.__name.set('');
		}
	}
	updateStateVars(params: ReusableRow_Params) {
		this.__name.reset(params.name);
	}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__name.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__name.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	updateRecycleElmtId(oldElmtId, newElmtId) {
		this.__name.updateElmtId(oldElmtId, newElmtId);
	}
	private __name: SynchedPropertySimpleOneWayPU<string>;
	get name() {
		return this.__name.get();
	}
	set name(newValue: string) {
		this.__name.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Row.create();
		}, Row);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(this.name);
		}, Text);
		Text.pop();
		Row.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class ShorthandProperty extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__name = new ObservedPropertySimplePU('n', this, "name");
        this.__ident = new ObservedPropertySimplePU('i', this, "ident");
        this.__items = new ObservedPropertyObjectPU([], this, "items");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ShorthandProperty_Params) {
        if (params.name !== undefined) {
            this.name = params.name;
        }
        if (params.ident !== undefined) {
            this.ident = params.ident;
        }
        if (params.items !== undefined) {
            this.items = params.items;
        }
    }
    updateStateVars(params: ShorthandProperty_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__name.purgeDependencyOnElmtId(rmElmtId);
        this.__ident.purgeDependencyOnElmtId(rmElmtId);
        this.__items.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__name.aboutToBeDeleted();
        this.__ident.aboutToBeDeleted();
        this.__items.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __name: ObservedPropertySimplePU<string>;
    get name() {
        return this.__name.get();
    }
    set name(newValue: string) {
        this.__name.set(newValue);
    }
    private __ident: ObservedPropertySimplePU<string>;
    get ident() {
        return this.__ident.get();
    }
    set ident(newValue: string) {
        this.__ident.set(newValue);
    }
    private __items: ObservedPropertyObjectPU<Pair[]>;
    get items() {
        return this.__items.get();
    }
    set items(newValue: Pair[]) {
        this.__items.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                Text.create(this.describe());
            }, Text);
            Text.pop();
            {
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    if (isInitialRender) {
                        let componentCall = new ChildRow(this, { name: this.name, tag: this.ident }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 50, col: 7 });
                        ViewPU.create(componentCall);
                        let paramsLambda = () => {
                            return { name: this.name, tag: this.ident };
                        };
                        componentCall.paramsGenerator_ = paramsLambda;
                    }
                    else {
                        this.updateStateVarsOfChildByElmtId(elmtId, { name: this.name, tag: this.ident });
                    }
                }, { name: "ChildRow" });
            }
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                __Recycle__.create();
            }, __Recycle__);
            {
                this.observeRecycleComponentCreation("ReusableRow", (elmtId, isInitialRender, recycleNode = null) => {
                    ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
                    if (isInitialRender) {
                        let componentCall = recycleNode ? recycleNode : new ReusableRow(this, { name: this.name }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 51, col: 7 });
                        ViewPU.createRecycle(componentCall, recycleNode !== null, "ReusableRow", () => {
                            if (recycleNode && typeof recycleNode.aboutToReuseInternal === "function") {
                                recycleNode.aboutToReuseInternal();
                            }
                            else {
                                if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === "function") {
                                    recycleNode.aboutToReuse({ name: this.name });
                                }
                                recycleNode.rerender();
                            }
                        });
                        let paramsLambda = () => {
                            return { name: this.name };
                        };
                        componentCall.paramsGenerator_ = paramsLambda;
                    }
                    else {
                        this.updateStateVarsOfChildByElmtId(elmtId, { name: this.name });
                    }
                    ViewStackProcessor.StopGetAccessRecording();
                });
            }
            __Recycle__.pop();
        Column.pop();
    }
describe(): string {
    const name = this.name
    const key = 'k'
    const value = 1
    // 1. 键与值同名
    const same: Pair = { name: name, key: key, value: value }
    // 2. 键与值不同名
    const other: Pair = { name: key, key: name, value: value }
    // 3. 值是成员访问
    const member: Pair = { name: this.name, key: this.ident, value: value }
    // 4. 值是调用
    const called: Pair = { name: getFn(), key: getFn(), value: value }
    // 5. 嵌套对象字面量里的简写
    const nested: Nested = { inner: { name: name, key: key, value: value } }
    // 6. 箭头函数返回对象里的简写
    const make = (): Pair => ({ name: name, key: key, value: value })
    // 7. 数组元素是对象字面量时的简写
    const list: Pair[] = [{ name: name, key: key, value: value }]
    this.items = list
    return `${same.name}${other.name}${member.name}${called.name}${nested.inner.name}${make().name}`
  }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "ShorthandProperty";
    }
}
registerNamedRoute(() => new ShorthandProperty(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
