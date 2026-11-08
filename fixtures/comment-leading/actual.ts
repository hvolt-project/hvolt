if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface CommentLeading_Params {
    cond?: boolean;
    elseCond?: boolean;
    items?: string[];
    source?: ProbeDataSource;
    dlg?: CustomDialogController;
    gapFirst?: number;
    // GAP_AFTER_METHODS
    plainAfterGap?: number;
    objFirst?: ObjKind;
    // AFTER_OBJ
    plainAfterObj?: number;
    objIdentFirst?: ObjKind;
    // BEFORE_OBJ_IDENT
    plainAfterObjIdent?: number;
    identFirst?: number;
    // BEFORE_IDENT
    plainAfterIdent?: number;
    multiFirst?: number;
    // BEFORE_MULTI_OBJ
    objMulti?: ObjKind;
    numFirst?: number;
    // BEFORE_OBJ
    objAfter?: ObjKind;
    /* MEMBER_PLAIN */
    dlg2?: CustomDialogController;
    dlg3?: CustomDialogController;
    tagged?: number;
}
interface AttrChainProbe_Params {
	wide?: boolean;
}
interface ConsumeFirstProbe_Params {
	firstField?: string;
}
interface ConsumeCarryProbe_Params {
	first?: number;
	adjacent?: string;
	afterMethod?: string;
	afterBuild?: string;
}
interface ProbeDialog_Params {
	controller?: CustomDialogController;
	title?: ResourceStr;
}
// Where the reference puts a comment written at each of five positions. Every marker is unique so
// `grep -c` in `golden/Index.ts` decides whether the position survives and how many copies it gets.
// The expected placements are recorded in README.md; this file is the acceptance fixture for the
// comment-emission path, so a fix is judged by those counts rather than by eyeballing a diff.

class ProbeDataSource implements IDataSource {
    private listeners: DataChangeListener[] = []
    private data: string[] = []

    totalCount(): number {
        return this.data.length
    }

    getData(index: number): string {
        return this.data[index]
    }

    registerDataChangeListener(listener: DataChangeListener): void {
        this.listeners.push(listener)
    }

    unregisterDataChangeListener(listener: DataChangeListener): void {
        const pos = this.listeners.indexOf(listener)
        if (pos >= 0) {
            this.listeners.splice(pos, 1)
        }
    }
}
const INIT_VALUE: number = 9;
const INIT_OBJ: ObjKind = { a: 3 };
interface ObjKind {
	a: number;
}
enum LocalKind {
	A = 0,
	B = 1
}
class ProbeDialog extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.controller = undefined;
		this.__title = new SynchedPropertyObjectOneWayPU(params.title, this, 'title');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: ProbeDialog_Params) {
		if (params.controller !== undefined) {
			this.controller = params.controller;
		}
	}
	updateStateVars(params: ProbeDialog_Params) {
		this.__title.reset(params.title);
	}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__title.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__title.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private controller?: CustomDialogController;
	setController(ctr: CustomDialogController) {
		this.controller = ctr;
	}
	private __title: SynchedPropertySimpleOneWayPU<ResourceStr>;
	get title() {
		return this.__title.get();
	}
	set title(newValue: ResourceStr) {
		this.__title.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
}
// ANIM_TOP
function animWidth(width: number, elmtId, isInitialRender, parent): void {
    if (isInitialRender) {
        Text.createAnimatableProperty("animWidth", width, (width: number) => {
            ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
            ViewStackProcessor.GetAndPushFrameNode("Text", elmtId);
            Text.width(width);
            ViewStackProcessor.StopGetAccessRecording();
            parent.finishUpdateFunc(elmtId);
        });
        Text.width(width);
    }
    else {
        Text.updateAnimatableProperty("animWidth", width);
    }
}
// EXTEND_TOP
function __Text__loudExtend(): void {
    Text.fontWeight(FontWeight.Bold);
}
class ConsumeCarryProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__first = new ObservedPropertySimplePU(1
        // CARRY_ADJACENT
        , this, "first");
        this.__adjacent = this.initializeConsume("adjacent", "adjacent");
        this.__afterMethod = this.initializeConsume("afterMethod", "afterMethod");
        this.__afterBuild = this.initializeConsume("afterBuild", "afterBuild");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ConsumeCarryProbe_Params) {
        if (params.first !== undefined) {
            this.first = params.first;
        }
    }
    updateStateVars(params: ConsumeCarryProbe_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__first.purgeDependencyOnElmtId(rmElmtId);
        this.__adjacent.purgeDependencyOnElmtId(rmElmtId);
        this.__afterMethod.purgeDependencyOnElmtId(rmElmtId);
        this.__afterBuild.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__first.aboutToBeDeleted();
        this.__adjacent.aboutToBeDeleted();
        this.__afterMethod.aboutToBeDeleted();
        this.__afterBuild.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __first: ObservedPropertySimplePU<number>;
    get first() {
        return this.__first.get();
    }
    set first(newValue: number) {
        this.__first.set(newValue);
    }
    // CARRY_ADJACENT
    private __adjacent: ObservedPropertyAbstractPU<string>;
    get adjacent() {
        return this.__adjacent.get();
    }
    set adjacent(newValue: string) {
        this.__adjacent.set(newValue);
    }
    helperMethod(): number {
        return 1
    }
    // CARRY_AFTER_METHOD
    private __afterMethod: ObservedPropertyAbstractPU<string>;
    get afterMethod() {
        return this.__afterMethod.get();
    }
    set afterMethod(newValue: string) {
        this.__afterMethod.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        Column.pop();
    }
    // CARRY_AFTER_BUILD
    private __afterBuild: ObservedPropertyAbstractPU<string>;
    get afterBuild() {
        return this.__afterBuild.get();
    }
    set afterBuild(newValue: string) {
        this.__afterBuild.set(newValue);
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class ConsumeFirstProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__firstField = this.initializeConsume("firstField", "firstField");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ConsumeFirstProbe_Params) {
    }
    updateStateVars(params: ConsumeFirstProbe_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__firstField.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__firstField.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    // CARRY_FIRST
    private __firstField: ObservedPropertyAbstractPU<string>;
    get firstField() {
        return this.__firstField.get();
    }
    set firstField(newValue: string) {
        this.__firstField.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class AttrChainProbe extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__wide = new ObservedPropertySimplePU(true, this, "wide");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: AttrChainProbe_Params) {
        if (params.wide !== undefined) {
            this.wide = params.wide;
        }
    }
    updateStateVars(params: AttrChainProbe_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__wide.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__wide.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    updateRecycleElmtId(oldElmtId, newElmtId) {
        this.__wide.updateElmtId(oldElmtId, newElmtId);
    }
    private __wide: ObservedPropertySimplePU<boolean>;
    get wide() {
        return this.__wide.get();
    }
    set wide(newValue: boolean) {
        this.__wide.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // ATTR_REUSABLE
                Text.create('r');
                // ATTR_REUSABLE
                Text.width(this.wide ? 10 : 20);
                if (isInitialRender) {
                    // ATTR_REUSABLE
                    Text.fontSize(12);
                    // ATTR_REUSABLE
                    Text.fontColor(Color.Red);
                }
            }, Text);
            // ATTR_REUSABLE
            Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class CommentLeading extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__cond = new ObservedPropertySimplePU(true, this, "cond");
        this.__elseCond = new ObservedPropertySimplePU(false, this, "elseCond");
        this.__items = new ObservedPropertyObjectPU(['x'], this, "items");
        this.source = new ProbeDataSource();
        this.dlg = new CustomDialogController({
    builder: () => {
                let jsDialog = new ProbeDialog(this, { title: 'probe' }, undefined, -1, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 126, col: 14 });
                jsDialog.setController(this.dlg);
                ViewPU.create(jsDialog);
                let paramsLambda = () => {
                    return { title: 'probe' };
                };
                jsDialog.paramsGenerator_ = paramsLambda;
            },
    autoCancel: true
  }, this);
        this.__gapFirst = new ObservedPropertySimplePU(17, this, "gapFirst");
        this.plainAfterGap = 19;
        this.__objFirst = new ObservedPropertyObjectPU({ a: 1 }
        // AFTER_OBJ
        , this, "objFirst");
        this.plainAfterObj = 7;
        this.__objIdentFirst = new ObservedPropertyObjectPU(INIT_OBJ
        // BEFORE_OBJ_IDENT
        , this, "objIdentFirst");
        this.plainAfterObjIdent = 13;
        this.__identFirst = new ObservedPropertySimplePU(INIT_VALUE
        // BEFORE_IDENT
        , this, "identFirst");
        this.plainAfterIdent = 11;
        this.__multiFirst = new ObservedPropertySimplePU(5
        // BEFORE_MULTI_OBJ
        , this, "multiFirst");
        this.objMulti = {
    a: 2
  };
        this.__numFirst = new ObservedPropertySimplePU(3
        // BEFORE_OBJ
        , this, "numFirst");
        this.objAfter = { a: 1 };
        this.dlg2 = new CustomDialogController({
    builder: () => {
                let jsDialog = new ProbeDialog(this, { title: 'plain' }, undefined, -1, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 163, col: 14 });
                jsDialog.setController(this.
                /* MEMBER_PLAIN */
                dlg2);
                ViewPU.create(jsDialog);
                let paramsLambda = () => {
                    return { title: 'plain' };
                };
                jsDialog.paramsGenerator_ = paramsLambda;
            },
    autoCancel: true
  }, this);
        this.dlg3 = new CustomDialogController({
    builder: () => {
                let jsDialog = new ProbeDialog(this, { title: 'public' }, undefined, -1, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 169, col: 14 });
                jsDialog.setController(this.dlg3);
                ViewPU.create(jsDialog);
                let paramsLambda = () => {
                    return { title: 'public' };
                };
                jsDialog.paramsGenerator_ = paramsLambda;
            },
    autoCancel: true
  }, this);
        this.__tagged = new ObservedPropertySimplePU(1, this, "tagged");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: CommentLeading_Params) {
        if (params.cond !== undefined) {
            this.cond = params.cond;
        }
        if (params.elseCond !== undefined) {
            this.elseCond = params.elseCond;
        }
        if (params.items !== undefined) {
            this.items = params.items;
        }
        if (params.source !== undefined) {
            this.source = params.source;
        }
        if (params.dlg !== undefined) {
            this.dlg = params.dlg;
        }
        if (params.gapFirst !== undefined) {
            this.gapFirst = params.gapFirst;
        }
        if (params.plainAfterGap !== undefined) {
            this.plainAfterGap = params.plainAfterGap;
        }
        if (params.objFirst !== undefined) {
            this.objFirst = params.objFirst;
        }
        if (params.plainAfterObj !== undefined) {
            this.plainAfterObj = params.plainAfterObj;
        }
        if (params.objIdentFirst !== undefined) {
            this.objIdentFirst = params.objIdentFirst;
        }
        if (params.plainAfterObjIdent !== undefined) {
            this.plainAfterObjIdent = params.plainAfterObjIdent;
        }
        if (params.identFirst !== undefined) {
            this.identFirst = params.identFirst;
        }
        if (params.plainAfterIdent !== undefined) {
            this.plainAfterIdent = params.plainAfterIdent;
        }
        if (params.multiFirst !== undefined) {
            this.multiFirst = params.multiFirst;
        }
        if (params.objMulti !== undefined) {
            this.objMulti = params.objMulti;
        }
        if (params.numFirst !== undefined) {
            this.numFirst = params.numFirst;
        }
        if (params.objAfter !== undefined) {
            this.objAfter = params.objAfter;
        }
        if (params.dlg2 !== undefined) {
            this.dlg2 = params.dlg2;
        }
        if (params.dlg3 !== undefined) {
            this.dlg3 = params.dlg3;
        }
        if (params.tagged !== undefined) {
            this.tagged = params.tagged;
        }
    }
    updateStateVars(params: CommentLeading_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__cond.purgeDependencyOnElmtId(rmElmtId);
        this.__elseCond.purgeDependencyOnElmtId(rmElmtId);
        this.__items.purgeDependencyOnElmtId(rmElmtId);
        this.__gapFirst.purgeDependencyOnElmtId(rmElmtId);
        this.__objFirst.purgeDependencyOnElmtId(rmElmtId);
        this.__objIdentFirst.purgeDependencyOnElmtId(rmElmtId);
        this.__identFirst.purgeDependencyOnElmtId(rmElmtId);
        this.__multiFirst.purgeDependencyOnElmtId(rmElmtId);
        this.__numFirst.purgeDependencyOnElmtId(rmElmtId);
        this.__tagged.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__cond.aboutToBeDeleted();
        this.__elseCond.aboutToBeDeleted();
        this.__items.aboutToBeDeleted();
        this.__gapFirst.aboutToBeDeleted();
        this.__objFirst.aboutToBeDeleted();
        this.__objIdentFirst.aboutToBeDeleted();
        this.__identFirst.aboutToBeDeleted();
        this.__multiFirst.aboutToBeDeleted();
        this.__numFirst.aboutToBeDeleted();
        this.__tagged.aboutToBeDeleted();
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
    private __elseCond: ObservedPropertySimplePU<boolean>;
    get elseCond() {
        return this.__elseCond.get();
    }
    set elseCond(newValue: boolean) {
        this.__elseCond.set(newValue);
    }
    private __items: ObservedPropertyObjectPU<string[]>;
    get items() {
        return this.__items.get();
    }
    set items(newValue: string[]) {
        this.__items.set(newValue);
    }
    private source: ProbeDataSource;
    /* 自定义弹框 */
    // controller 定义
    private dlg: CustomDialogController;
    private __gapFirst: ObservedPropertySimplePU<number>;
    get gapFirst() {
        return this.__gapFirst.get();
    }
    set gapFirst(newValue: number) {
        this.__gapFirst.set(newValue);
    }
    helperGap(): number {
        return 1
    }
    // GAP_AFTER_METHODS
    private plainAfterGap: number;
    private __objFirst: ObservedPropertyObjectPU<ObjKind>;
    get objFirst() {
        return this.__objFirst.get();
    }
    set objFirst(newValue: ObjKind) {
        this.__objFirst.set(newValue);
    }
    // AFTER_OBJ
    private plainAfterObj: number;
    private __objIdentFirst: ObservedPropertyObjectPU<ObjKind>;
    get objIdentFirst() {
        return this.__objIdentFirst.get();
    }
    set objIdentFirst(newValue: ObjKind) {
        this.__objIdentFirst.set(newValue);
    }
    // BEFORE_OBJ_IDENT
    private plainAfterObjIdent: number;
    private __identFirst: ObservedPropertySimplePU<number>;
    get identFirst() {
        return this.__identFirst.get();
    }
    set identFirst(newValue: number) {
        this.__identFirst.set(newValue);
    }
    // BEFORE_IDENT
    private plainAfterIdent: number;
    private __multiFirst: ObservedPropertySimplePU<number>;
    get multiFirst() {
        return this.__multiFirst.get();
    }
    set multiFirst(newValue: number) {
        this.__multiFirst.set(newValue);
    }
    // BEFORE_MULTI_OBJ
    private objMulti: ObjKind;
    private __numFirst: ObservedPropertySimplePU<number>;
    get numFirst() {
        return this.__numFirst.get();
    }
    set numFirst(newValue: number) {
        this.__numFirst.set(newValue);
    }
    // BEFORE_OBJ
    private objAfter: ObjKind;
    /* MEMBER_PLAIN */
    private dlg2: CustomDialogController;
    /* MEMBER_PUBLIC */
    public dlg3: CustomDialogController;
    /* MEMBER_STATE */
    // MEMBER_STATE_SECOND
    private __tagged: ObservedPropertySimplePU<number>;
    get tagged() {
        return this.__tagged.get();
    }
    set tagged(newValue: number) {
        this.__tagged.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                If.create();
                // IF_START
                if (this.cond) {
                    this.ifElseBranchUpdateFunction(0, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create('a');
                        }, Text);
                        Text.pop();
                    });
                }
                else {
                    this.ifElseBranchUpdateFunction(1, () => {
                    });
                }
            }, If);
            If.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                If.create();
                if (this.elseCond) {
                    this.ifElseBranchUpdateFunction(0, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create('no');
                        }, Text);
                        Text.pop();
                    });
                }
                else {
                    this.ifElseBranchUpdateFunction(1, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            // ELSE_FIRST
                            Text.create('b');
                        }, Text);
                        // ELSE_FIRST
                        Text.pop();
                    });
                }
            }, If);
            If.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // FOREACH_START
                ForEach.create();
                const forEachItemGenFunction = (_item) => {
                    const item = _item;
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create(item);
                    }, Text);
                    Text.pop();
                };
                this.forEachUpdateFunction(elmtId, this.items, forEachItemGenFunction, (item: string) => item, false, false);
            }, ForEach);
            // FOREACH_START
            ForEach.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                If.create();
                if (this.cond) {
                    this.ifElseBranchUpdateFunction(0, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create('c');
                        }, Text);
                        Text.pop();
                    });
                }
                // AFTER_BLOCK
                else {
                    this.ifElseBranchUpdateFunction(1, () => {
                    });
                }
            }, If);
            If.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // AFTER_BLOCK
                Text.create('d');
            }, Text);
            // AFTER_BLOCK
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // BLOCK_ONE
                // BLOCK_TWO
                Text.create('x');
            }, Text);
            // BLOCK_ONE
            // BLOCK_TWO
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                /* BLOCK_A */
                // BLOCK_B
                Text.create('y');
            }, Text);
            /* BLOCK_A */
            // BLOCK_B
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // TRIO_ONE
                // TRIO_TWO
                // TRIO_THREE
                Text.create('z');
            }, Text);
            // TRIO_ONE
            // TRIO_TWO
            // TRIO_THREE
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                If.create();
                // BRANCH_IF
                if (this.cond) {
                    this.ifElseBranchUpdateFunction(0, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create('a');
                        }, Text);
                        Text.pop();
                    });
                }
                // BRANCH_ELSEIF
                else if (this.elseCond) {
                    this.ifElseBranchUpdateFunction(1, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create('b');
                        }, Text);
                        Text.pop();
                    });
                }
                // BRANCH_ELSE
                else {
                    this.ifElseBranchUpdateFunction(2, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create('c');
                        }, Text);
                        Text.pop();
                    });
                }
            }, If);
            If.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                If.create();
                if (this.cond) {
                    this.ifElseBranchUpdateFunction(0, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create('d');
                        }, Text);
                        Text.pop();
                    });
                }
                // BETWEEN_BRANCH
                else {
                    this.ifElseBranchUpdateFunction(1, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create('e');
                        }, Text);
                        Text.pop();
                    });
                }
            }, If);
            If.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // ANIM_CALL
                Text.create('an1');
                // ANIM_CALL
                Text.fontSize(8);
                animWidth(20, elmtId, isInitialRender, this);
            }, Text);
            // ANIM_CALL
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // EXTEND_CALL
                Text.create('e1');
                // EXTEND_CALL
                Text.fontSize(9);
                __Text__loudExtend();
            }, Text);
            // EXTEND_CALL
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // ATTR_ONE
                Text.create('a1');
                // ATTR_ONE
                Text.fontSize(12);
            }, Text);
            // ATTR_ONE
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // ATTR_TWO
                Text.create('a2');
                // ATTR_TWO
                Text.fontSize(12);
                // ATTR_TWO
                Text.fontColor(Color.Red);
            }, Text);
            // ATTR_TWO
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // ATTR_FOUR
                Text.create('a4');
                // ATTR_FOUR
                Text.fontSize(12);
                // ATTR_FOUR
                Text.fontColor(Color.Red);
                // ATTR_FOUR
                Text.width(100);
                // ATTR_FOUR
                Text.height(20);
            }, Text);
            // ATTR_FOUR
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // ATTR_COND
                Text.create('cond');
                // ATTR_COND
                Text.fontSize(this.cond ? 12 : 14);
                // ATTR_COND
                Text.width(100);
            }, Text);
            // ATTR_COND
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // ATTR_RES
                Text.create('res');
                // ATTR_RES
                Text.fontColor({ "id": 125830982, "type": 10001, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
                // ATTR_RES
                Text.fontSize(12);
            }, Text);
            // ATTR_RES
            Text.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                __Recycle__.create();
            }, __Recycle__);
            {
                this.observeRecycleComponentCreation("AttrChainProbe", (elmtId, isInitialRender, recycleNode = null) => {
                    ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
                    if (isInitialRender) {
                        let componentCall = recycleNode ? recycleNode : new AttrChainProbe(this, {}, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 282, col: 7 });
                        ViewPU.createRecycle(componentCall, recycleNode !== null, "AttrChainProbe", () => {
                            if (recycleNode && typeof recycleNode.aboutToReuseInternal === "function") {
                                recycleNode.aboutToReuseInternal();
                            }
                            else {
                                if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === "function") {
                                    recycleNode.aboutToReuse({});
                                }
                                recycleNode.rerender();
                            }
                        });
                        let paramsLambda = () => {
                            return {};
                        };
                        componentCall.paramsGenerator_ = paramsLambda;
                    }
                    else {
                        this.updateStateVarsOfChildByElmtId(elmtId, {});
                    }
                    ViewStackProcessor.StopGetAccessRecording();
                });
            }
            __Recycle__.pop();
            {
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    if (isInitialRender) {
                        let componentCall = new ConsumeCarryProbe(this, {}, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 283, col: 7 });
                        ViewPU.create(componentCall);
                        let paramsLambda = () => {
                            return {};
                        };
                        componentCall.paramsGenerator_ = paramsLambda;
                    }
                    else {
                        this.updateStateVarsOfChildByElmtId(elmtId, {});
                    }
                }, { name: "ConsumeCarryProbe" });
            }
            {
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    if (isInitialRender) {
                        let componentCall = new ConsumeFirstProbe(this, {}, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 284, col: 7 });
                        ViewPU.create(componentCall);
                        let paramsLambda = () => {
                            return {};
                        };
                        componentCall.paramsGenerator_ = paramsLambda;
                    }
                    else {
                        this.updateStateVarsOfChildByElmtId(elmtId, {});
                    }
                }, { name: "ConsumeFirstProbe" });
            }
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                If.create();
                if (this.cond) {
                    this.ifElseBranchUpdateFunction(0, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            __Recycle__.create();
                        }, __Recycle__);
                        {
                            this.observeRecycleComponentCreation("AttrChainProbe", (elmtId, isInitialRender, recycleNode = null) => {
                                ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
                                if (isInitialRender) {
                                    let componentCall = recycleNode ? recycleNode : new 
                                    // CHILD_CALL
                                    AttrChainProbe(this, {}, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 288, col: 9 });
                                    ViewPU.createRecycle(componentCall, recycleNode !== null, "AttrChainProbe", () => {
                                        if (recycleNode && typeof recycleNode.aboutToReuseInternal === "function") {
                                            recycleNode.aboutToReuseInternal();
                                        }
                                        else {
                                            if (recycleNode.aboutToReuse && typeof recycleNode.aboutToReuse === "function") {
                                                recycleNode.aboutToReuse({});
                                            }
                                            recycleNode.rerender();
                                        }
                                    });
                                    let paramsLambda = () => {
                                        return {};
                                    };
                                    componentCall.paramsGenerator_ = paramsLambda;
                                }
                                else {
                                    this.updateStateVarsOfChildByElmtId(elmtId, {});
                                }
                                ViewStackProcessor.StopGetAccessRecording();
                            });
                        }
                        __Recycle__.pop();
                    });
                }
                else {
                    this.ifElseBranchUpdateFunction(1, () => {
                    });
                }
            }, If);
            If.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                List.create();
            }, List);
                {
                    const __lazyForEachItemGenFunction = (_item) => {
                        const item = _item;
                        {
                            const itemCreation2 = (elmtId, isInitialRender) => {
                                ListItem.create(() => { }, false);
                            };
                            const observedDeepRender = () => {
                                this.observeComponentCreation2(itemCreation2, ListItem);
                                this.observeComponentCreation2((elmtId, isInitialRender) => {
                                    Text.create(item);
                                }, Text);
                                Text.pop();
                                ListItem.pop();
                            };
                            observedDeepRender();
                        }
                    };
                    const __lazyForEachItemIdFunc = (item: string) => item;
                    LazyForEach.create("1", this, this.source, __lazyForEachItemGenFunction, __lazyForEachItemIdFunc);
                    // LAZY_START
                    LazyForEach.pop();
                }
            List.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                If.create();
                if (this.elseCond) {
                    this.ifElseBranchUpdateFunction(0, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create('tail');
                        }, Text);
                        Text.pop();
                    });
                }
                // IF_TAIL
                else {
                    this.ifElseBranchUpdateFunction(1, () => {
                    });
                }
            }, If);
            If.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "CommentLeading";
    }
}
registerNamedRoute(() => new CommentLeading(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
