if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface ObservedPropertyKind_Params {
	consumedConfig?: AppConfig;
	consumedNumber?: number;
	count?: number;
	label?: string;
	flag?: boolean;
	config?: AppConfig;
	item?: SomeClass;
	list?: number[];
	objs?: SomeClass[];
	m?: Map<string, number>;
	maybe?: string | undefined;
	maybeBool?: boolean | undefined;
	opt?: number;
	kind?: LocalKind;
	localAlias?: LocalAlias;
	numberAlias?: NumberAlias;
	stringPrimAlias?: StringPrimAlias;
	boolAlias?: BoolAlias;
	enumAlias?: EnumAlias;
	twoPrimAlias?: TwoPrimAlias;
	inlineLiteralUnion?: 'a' | 'b';
	inlinePrimUnion?: number | string;
	iface?: SomeInterface;
	angleList?: Array<number>;
}
interface ConsumeHost_Params {
	consumedConfig?: AppConfig;
	consumedNumber?: number;
}
interface PropHost_Params {
	propObject?: SomeClass;
	propNumber?: number;
	linkObject?: AppConfig;
	objectLink?: SomeClass;
}
// 状态字段的 backstore 类型由字段类型决定：哪一类是 `ObservedPropertySimplePU`、
// 哪一类是 `ObservedPropertyObjectPU`；`@Prop`/`@Link` 那族同理（Simple/Object OneWay）。
// 本地类型别名：字符串字面量联合 / number
type LocalAlias = 'a' | 'b'
type NumberAlias = number;
type StringPrimAlias = string;
type BoolAlias = boolean;
type EnumAlias = LocalKind;
type TwoPrimAlias = string | number;
interface SomeInterface {
	name: string;
}
class AppConfig {
	name: string = '';
}
@Observed class SomeClass {
	value: number = 0;
	name: string = '';
}
enum LocalKind {
	A = 1,
	B = 2
}
class PropHost extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__propObject = new SynchedPropertyObjectOneWayPU(params.propObject, this, 'propObject');
		this.__propNumber = new SynchedPropertySimpleOneWayPU(params.propNumber, this, 'propNumber');
		this.__linkObject = new SynchedPropertyObjectTwoWayPU(params.linkObject, this, 'linkObject');
		this.__objectLink = new SynchedPropertyNesedObjectPU(params.objectLink, this, 'objectLink');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: PropHost_Params) {
		if (params.propObject === undefined) {
			this.__propObject.set(new SomeClass());
		}
		if (params.propNumber === undefined) {
			this.__propNumber.set(0);
		}
		this.__objectLink.set(params.objectLink);
	}
	updateStateVars(params: PropHost_Params) {
		this.__propObject.reset(params.propObject);
		this.__propNumber.reset(params.propNumber);
		this.__objectLink.set(params.objectLink);
	}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__propObject.purgeDependencyOnElmtId(rmElmtId);
		this.__propNumber.purgeDependencyOnElmtId(rmElmtId);
		this.__linkObject.purgeDependencyOnElmtId(rmElmtId);
		this.__objectLink.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__propObject.aboutToBeDeleted();
		this.__propNumber.aboutToBeDeleted();
		this.__linkObject.aboutToBeDeleted();
		this.__objectLink.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __propObject: SynchedPropertySimpleOneWayPU<SomeClass>;
	get propObject() {
		return this.__propObject.get();
	}
	set propObject(newValue: SomeClass) {
		this.__propObject.set(newValue);
	}
	private __propNumber: SynchedPropertySimpleOneWayPU<number>;
	get propNumber() {
		return this.__propNumber.get();
	}
	set propNumber(newValue: number) {
		this.__propNumber.set(newValue);
	}
	private __linkObject: SynchedPropertySimpleOneWayPU<AppConfig>;
	get linkObject() {
		return this.__linkObject.get();
	}
	set linkObject(newValue: AppConfig) {
		this.__linkObject.set(newValue);
	}
	private __objectLink: SynchedPropertyNesedObjectPU<SomeClass>;
	get objectLink() {
		return this.__objectLink.get();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.propNumber}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class ConsumeHost extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__consumedConfig = this.initializeConsume('consumedConfig', 'consumedConfig');
		this.__consumedNumber = this.initializeConsume('consumedNumber', 'consumedNumber');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ConsumeHost_Params) {}
	updateStateVars(params: ConsumeHost_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__consumedConfig.purgeDependencyOnElmtId(rmElmtId);
		this.__consumedNumber.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__consumedConfig.aboutToBeDeleted();
		this.__consumedNumber.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __consumedConfig: ObservedPropertyAbstractPU<AppConfig>;
	get consumedConfig() {
		return this.__consumedConfig.get();
	}
	set consumedConfig(newValue: AppConfig) {
		this.__consumedConfig.set(newValue);
	}
	private __consumedNumber: ObservedPropertyAbstractPU<number>;
	get consumedNumber() {
		return this.__consumedNumber.get();
	}
	set consumedNumber(newValue: number) {
		this.__consumedNumber.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${this.consumedNumber}`);
		}, Text);
		Text.pop();
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
class ObservedPropertyKind extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__consumedConfig = new ObservedPropertyObjectPU(new AppConfig(), this, "consumedConfig");
        this.addProvidedVar("consumedConfig", this.__consumedConfig, false);
        this.__consumedNumber = new ObservedPropertySimplePU(0, this, "consumedNumber");
        this.addProvidedVar("consumedNumber", this.__consumedNumber, false);
        this.__count = new ObservedPropertySimplePU(0, this, "count");
        this.__label = new ObservedPropertySimplePU('a', this, "label");
        this.__flag = new ObservedPropertySimplePU(false, this, "flag");
        this.__config = new ObservedPropertyObjectPU({ name: 'x' }, this, "config");
        this.__item = new ObservedPropertyObjectPU(new SomeClass(), this, "item");
        this.__list = new ObservedPropertyObjectPU([], this, "list");
        this.__objs = new ObservedPropertyObjectPU([], this, "objs");
        this.__m = new ObservedPropertyObjectPU(new Map(), this, "m");
        this.__maybe = new ObservedPropertyObjectPU(undefined, this, "maybe");
        this.__maybeBool = new ObservedPropertySimplePU(false, this, "maybeBool");
        this.__opt = new ObservedPropertySimplePU(undefined, this, "opt");
        this.__kind = new ObservedPropertySimplePU(LocalKind.A, this, "kind");
        this.__localAlias = new ObservedPropertySimplePU('a', this, "localAlias");
        this.__numberAlias = new ObservedPropertyObjectPU(1, this, "numberAlias");
        this.__stringPrimAlias = new ObservedPropertyObjectPU('s', this, "stringPrimAlias");
        this.__boolAlias = new ObservedPropertySimplePU(true, this, "boolAlias");
        this.__enumAlias = new ObservedPropertySimplePU(LocalKind.A, this, "enumAlias");
        this.__twoPrimAlias = new ObservedPropertySimplePU('s', this, "twoPrimAlias");
        this.__inlineLiteralUnion = new ObservedPropertySimplePU('a', this, "inlineLiteralUnion");
        this.__inlinePrimUnion = new ObservedPropertySimplePU(1, this, "inlinePrimUnion");
        this.__iface = new ObservedPropertyObjectPU({ name: 'n' }, this, "iface");
        this.__angleList = new ObservedPropertyObjectPU([], this, "angleList");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ObservedPropertyKind_Params) {
        if (params.consumedConfig !== undefined) {
            this.consumedConfig = params.consumedConfig;
        }
        if (params.consumedNumber !== undefined) {
            this.consumedNumber = params.consumedNumber;
        }
        if (params.count !== undefined) {
            this.count = params.count;
        }
        if (params.label !== undefined) {
            this.label = params.label;
        }
        if (params.flag !== undefined) {
            this.flag = params.flag;
        }
        if (params.config !== undefined) {
            this.config = params.config;
        }
        if (params.item !== undefined) {
            this.item = params.item;
        }
        if (params.list !== undefined) {
            this.list = params.list;
        }
        if (params.objs !== undefined) {
            this.objs = params.objs;
        }
        if (params.m !== undefined) {
            this.m = params.m;
        }
        if (params.maybe !== undefined) {
            this.maybe = params.maybe;
        }
        if (params.maybeBool !== undefined) {
            this.maybeBool = params.maybeBool;
        }
        if (params.opt !== undefined) {
            this.opt = params.opt;
        }
        if (params.kind !== undefined) {
            this.kind = params.kind;
        }
        if (params.localAlias !== undefined) {
            this.localAlias = params.localAlias;
        }
        if (params.numberAlias !== undefined) {
            this.numberAlias = params.numberAlias;
        }
        if (params.stringPrimAlias !== undefined) {
            this.stringPrimAlias = params.stringPrimAlias;
        }
        if (params.boolAlias !== undefined) {
            this.boolAlias = params.boolAlias;
        }
        if (params.enumAlias !== undefined) {
            this.enumAlias = params.enumAlias;
        }
        if (params.twoPrimAlias !== undefined) {
            this.twoPrimAlias = params.twoPrimAlias;
        }
        if (params.inlineLiteralUnion !== undefined) {
            this.inlineLiteralUnion = params.inlineLiteralUnion;
        }
        if (params.inlinePrimUnion !== undefined) {
            this.inlinePrimUnion = params.inlinePrimUnion;
        }
        if (params.iface !== undefined) {
            this.iface = params.iface;
        }
        if (params.angleList !== undefined) {
            this.angleList = params.angleList;
        }
    }
    updateStateVars(params: ObservedPropertyKind_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__consumedConfig.purgeDependencyOnElmtId(rmElmtId);
        this.__consumedNumber.purgeDependencyOnElmtId(rmElmtId);
        this.__count.purgeDependencyOnElmtId(rmElmtId);
        this.__label.purgeDependencyOnElmtId(rmElmtId);
        this.__flag.purgeDependencyOnElmtId(rmElmtId);
        this.__config.purgeDependencyOnElmtId(rmElmtId);
        this.__item.purgeDependencyOnElmtId(rmElmtId);
        this.__list.purgeDependencyOnElmtId(rmElmtId);
        this.__objs.purgeDependencyOnElmtId(rmElmtId);
        this.__m.purgeDependencyOnElmtId(rmElmtId);
        this.__maybe.purgeDependencyOnElmtId(rmElmtId);
        this.__maybeBool.purgeDependencyOnElmtId(rmElmtId);
        this.__opt.purgeDependencyOnElmtId(rmElmtId);
        this.__kind.purgeDependencyOnElmtId(rmElmtId);
        this.__localAlias.purgeDependencyOnElmtId(rmElmtId);
        this.__numberAlias.purgeDependencyOnElmtId(rmElmtId);
        this.__stringPrimAlias.purgeDependencyOnElmtId(rmElmtId);
        this.__boolAlias.purgeDependencyOnElmtId(rmElmtId);
        this.__enumAlias.purgeDependencyOnElmtId(rmElmtId);
        this.__twoPrimAlias.purgeDependencyOnElmtId(rmElmtId);
        this.__inlineLiteralUnion.purgeDependencyOnElmtId(rmElmtId);
        this.__inlinePrimUnion.purgeDependencyOnElmtId(rmElmtId);
        this.__iface.purgeDependencyOnElmtId(rmElmtId);
        this.__angleList.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__consumedConfig.aboutToBeDeleted();
        this.__consumedNumber.aboutToBeDeleted();
        this.__count.aboutToBeDeleted();
        this.__label.aboutToBeDeleted();
        this.__flag.aboutToBeDeleted();
        this.__config.aboutToBeDeleted();
        this.__item.aboutToBeDeleted();
        this.__list.aboutToBeDeleted();
        this.__objs.aboutToBeDeleted();
        this.__m.aboutToBeDeleted();
        this.__maybe.aboutToBeDeleted();
        this.__maybeBool.aboutToBeDeleted();
        this.__opt.aboutToBeDeleted();
        this.__kind.aboutToBeDeleted();
        this.__localAlias.aboutToBeDeleted();
        this.__numberAlias.aboutToBeDeleted();
        this.__stringPrimAlias.aboutToBeDeleted();
        this.__boolAlias.aboutToBeDeleted();
        this.__enumAlias.aboutToBeDeleted();
        this.__twoPrimAlias.aboutToBeDeleted();
        this.__inlineLiteralUnion.aboutToBeDeleted();
        this.__inlinePrimUnion.aboutToBeDeleted();
        this.__iface.aboutToBeDeleted();
        this.__angleList.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    // 12. @Provide：对象与原始类型（backstore 种类应与 @State 同规）
    private __consumedConfig: ObservedPropertyObjectPU<AppConfig>;
    get consumedConfig() {
        return this.__consumedConfig.get();
    }
    set consumedConfig(newValue: AppConfig) {
        this.__consumedConfig.set(newValue);
    }
    private __consumedNumber: ObservedPropertySimplePU<number>;
    get consumedNumber() {
        return this.__consumedNumber.get();
    }
    set consumedNumber(newValue: number) {
        this.__consumedNumber.set(newValue);
    }
    // 1. 原始类型
    private __count: ObservedPropertySimplePU<number>;
    get count() {
        return this.__count.get();
    }
    set count(newValue: number) {
        this.__count.set(newValue);
    }
    private __label: ObservedPropertySimplePU<string>;
    get label() {
        return this.__label.get();
    }
    set label(newValue: string) {
        this.__label.set(newValue);
    }
    private __flag: ObservedPropertySimplePU<boolean>;
    get flag() {
        return this.__flag.get();
    }
    set flag(newValue: boolean) {
        this.__flag.set(newValue);
    }
    // 2. 对象字面量初值（本地 class）
    private __config: ObservedPropertyObjectPU<AppConfig>;
    get config() {
        return this.__config.get();
    }
    set config(newValue: AppConfig) {
        this.__config.set(newValue);
    }
    // 3. 类实例
    private __item: ObservedPropertyObjectPU<SomeClass>;
    get item() {
        return this.__item.get();
    }
    set item(newValue: SomeClass) {
        this.__item.set(newValue);
    }
    // 4. 数组
    private __list: ObservedPropertyObjectPU<number[]>;
    get list() {
        return this.__list.get();
    }
    set list(newValue: number[]) {
        this.__list.set(newValue);
    }
    private __objs: ObservedPropertyObjectPU<SomeClass[]>;
    get objs() {
        return this.__objs.get();
    }
    set objs(newValue: SomeClass[]) {
        this.__objs.set(newValue);
    }
    // 5. Map
    private __m: ObservedPropertyObjectPU<Map<string, number>>;
    get m() {
        return this.__m.get();
    }
    set m(newValue: Map<string, number>) {
        this.__m.set(newValue);
    }
    // 6. 联合与可选
    private __maybe: ObservedPropertyObjectPU<string | undefined>;
    get maybe() {
        return this.__maybe.get();
    }
    set maybe(newValue: string | undefined) {
        this.__maybe.set(newValue);
    }
    private __maybeBool: ObservedPropertySimplePU<boolean | undefined>;
    get maybeBool() {
        return this.__maybeBool.get();
    }
    set maybeBool(newValue: boolean | undefined) {
        this.__maybeBool.set(newValue);
    }
    private __opt?: ObservedPropertySimplePU<number>;
    get opt() {
        return this.__opt.get();
    }
    set opt(newValue: number) {
        this.__opt.set(newValue);
    }
    // 8. 枚举
    private __kind: ObservedPropertySimplePU<LocalKind>;
    get kind() {
        return this.__kind.get();
    }
    set kind(newValue: LocalKind) {
        this.__kind.set(newValue);
    }
    // 10. 本地类型别名
    private __localAlias: ObservedPropertySimplePU<LocalAlias>;
    get localAlias() {
        return this.__localAlias.get();
    }
    set localAlias(newValue: LocalAlias) {
        this.__localAlias.set(newValue);
    }
    private __numberAlias: ObservedPropertyObjectPU<NumberAlias>;
    get numberAlias() {
        return this.__numberAlias.get();
    }
    set numberAlias(newValue: NumberAlias) {
        this.__numberAlias.set(newValue);
    }
    private __stringPrimAlias: ObservedPropertyObjectPU<StringPrimAlias>;
    get stringPrimAlias() {
        return this.__stringPrimAlias.get();
    }
    set stringPrimAlias(newValue: StringPrimAlias) {
        this.__stringPrimAlias.set(newValue);
    }
    private __boolAlias: ObservedPropertySimplePU<BoolAlias>;
    get boolAlias() {
        return this.__boolAlias.get();
    }
    set boolAlias(newValue: BoolAlias) {
        this.__boolAlias.set(newValue);
    }
    private __enumAlias: ObservedPropertySimplePU<EnumAlias>;
    get enumAlias() {
        return this.__enumAlias.get();
    }
    set enumAlias(newValue: EnumAlias) {
        this.__enumAlias.set(newValue);
    }
    private __twoPrimAlias: ObservedPropertySimplePU<TwoPrimAlias>;
    get twoPrimAlias() {
        return this.__twoPrimAlias.get();
    }
    set twoPrimAlias(newValue: TwoPrimAlias) {
        this.__twoPrimAlias.set(newValue);
    }
    private __inlineLiteralUnion: ObservedPropertySimplePU<'a' | 'b'>;
    get inlineLiteralUnion() {
        return this.__inlineLiteralUnion.get();
    }
    set inlineLiteralUnion(newValue: 'a' | 'b') {
        this.__inlineLiteralUnion.set(newValue);
    }
    private __inlinePrimUnion: ObservedPropertySimplePU<number | string>;
    get inlinePrimUnion() {
        return this.__inlinePrimUnion.get();
    }
    set inlinePrimUnion(newValue: number | string) {
        this.__inlinePrimUnion.set(newValue);
    }
    // 11. interface / Array 角度括号
    private __iface: ObservedPropertyObjectPU<SomeInterface>;
    get iface() {
        return this.__iface.get();
    }
    set iface(newValue: SomeInterface) {
        this.__iface.set(newValue);
    }
    private __angleList: ObservedPropertyObjectPU<Array<number>>;
    get angleList() {
        return this.__angleList.get();
    }
    set angleList(newValue: Array<number>) {
        this.__angleList.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                Text.create(`${this.count}${this.label}${this.flag}${this.kind}`);
            }, Text);
            Text.pop();
            {
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    if (isInitialRender) {
                        let componentCall = new PropHost(this, { propObject: this.item, propNumber: 1, linkObject: this.__config, objectLink: this.item }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 97, col: 7 });
                        ViewPU.create(componentCall);
                        let paramsLambda = () => {
                            return { propObject: this.item, propNumber: 1, linkObject: this.config, objectLink: this.item };
                        };
                        componentCall.paramsGenerator_ = paramsLambda;
                    }
                    else {
                        this.updateStateVarsOfChildByElmtId(elmtId, { propObject: this.item, propNumber: 1, objectLink: this.item });
                    }
                }, { name: "PropHost" });
            }
            {
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    if (isInitialRender) {
                        let componentCall = new ConsumeHost(this, {}, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 98, col: 7 });
                        ViewPU.create(componentCall);
                        let paramsLambda = () => {
                            return {};
                        };
                        componentCall.paramsGenerator_ = paramsLambda;
                    }
                    else {
                        this.updateStateVarsOfChildByElmtId(elmtId, {});
                    }
                }, { name: "ConsumeHost" });
            }
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "ObservedPropertyKind";
    }
}
registerNamedRoute(() => new ObservedPropertyKind(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
