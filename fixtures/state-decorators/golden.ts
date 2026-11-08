if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface StateChild_Params {
    shared?: number;
    theme?: string;
    task?: Task;
    storageLinked?: number;
    storagePropped?: number;
    localStorageLinked?: number;
    localStoragePropped?: number;
}
interface StateHost_Params {
    count?: number;
    tasks?: Task[];
    current?: Task;
    theme?: string;
    plain?: string;
}
@Observed
class Task {
    title: string = '';
}
class StateHost extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__count = new ObservedPropertySimplePU(0, this, "count");
        this.__tasks = new ObservedPropertyObjectPU([], this, "tasks");
        this.__current = new ObservedPropertyObjectPU(new Task(), this, "current");
        this.__theme = new ObservedPropertySimplePU('light', this, "theme");
        this.addProvidedVar("theme", this.__theme, false);
        this.__plain = new ObservedPropertySimplePU('p', this, "plain");
        this.addProvidedVar("plain", this.__plain, false);
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: StateHost_Params) {
        if (params.count !== undefined) {
            this.count = params.count;
        }
        if (params.tasks !== undefined) {
            this.tasks = params.tasks;
        }
        if (params.current !== undefined) {
            this.current = params.current;
        }
        if (params.theme !== undefined) {
            this.theme = params.theme;
        }
        if (params.plain !== undefined) {
            this.plain = params.plain;
        }
    }
    updateStateVars(params: StateHost_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__count.purgeDependencyOnElmtId(rmElmtId);
        this.__tasks.purgeDependencyOnElmtId(rmElmtId);
        this.__current.purgeDependencyOnElmtId(rmElmtId);
        this.__theme.purgeDependencyOnElmtId(rmElmtId);
        this.__plain.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__count.aboutToBeDeleted();
        this.__tasks.aboutToBeDeleted();
        this.__current.aboutToBeDeleted();
        this.__theme.aboutToBeDeleted();
        this.__plain.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __count: ObservedPropertySimplePU<number>;
    get count() {
        return this.__count.get();
    }
    set count(newValue: number) {
        this.__count.set(newValue);
    }
    private __tasks: ObservedPropertyObjectPU<Task[]>;
    get tasks() {
        return this.__tasks.get();
    }
    set tasks(newValue: Task[]) {
        this.__tasks.set(newValue);
    }
    private __current: ObservedPropertyObjectPU<Task>;
    get current() {
        return this.__current.get();
    }
    set current(newValue: Task) {
        this.__current.set(newValue);
    }
    private __theme: ObservedPropertySimplePU<string>;
    get theme() {
        return this.__theme.get();
    }
    set theme(newValue: string) {
        this.__theme.set(newValue);
    }
    private __plain: ObservedPropertySimplePU<string>;
    get plain() {
        return this.__plain.get();
    }
    set plain(newValue: string) {
        this.__plain.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        {
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                if (isInitialRender) {
                    let componentCall = new StateChild(this, { shared: this.__count, task: this.current }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 17, col: 7 });
                    ViewPU.create(componentCall);
                    let paramsLambda = () => {
                        return {
                            shared: this.count,
                            task: this.current
                        };
                    };
                    componentCall.paramsGenerator_ = paramsLambda;
                }
                else {
                    this.updateStateVarsOfChildByElmtId(elmtId, {
                        task: this.current
                    });
                }
            }, { name: "StateChild" });
        }
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "StateHost";
    }
}
class StateChild extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__shared = new SynchedPropertySimpleTwoWayPU(params.shared, this, "shared");
        this.__theme = this.initializeConsume('theme', "theme");
        this.__task = new SynchedPropertyNesedObjectPU(params.task, this, "task");
        this.__storageLinked = this.createStorageLink('appKey', 0, "storageLinked");
        this.__storagePropped = this.createStorageProp('appKey', 0, "storagePropped");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: StateChild_Params) {
        this.__task.set(params.task);
    }
    updateStateVars(params: StateChild_Params) {
        this.__task.set(params.task);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__shared.purgeDependencyOnElmtId(rmElmtId);
        this.__theme.purgeDependencyOnElmtId(rmElmtId);
        this.__task.purgeDependencyOnElmtId(rmElmtId);
        this.__storageLinked.purgeDependencyOnElmtId(rmElmtId);
        this.__storagePropped.purgeDependencyOnElmtId(rmElmtId);
        this.__localStorageLinked.purgeDependencyOnElmtId(rmElmtId);
        this.__localStoragePropped.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__shared.aboutToBeDeleted();
        this.__theme.aboutToBeDeleted();
        this.__task.aboutToBeDeleted();
        this.__storageLinked.aboutToBeDeleted();
        this.__storagePropped.aboutToBeDeleted();
        this.__localStorageLinked.aboutToBeDeleted();
        this.__localStoragePropped.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __shared: SynchedPropertySimpleTwoWayPU<number>;
    get shared() {
        return this.__shared.get();
    }
    set shared(newValue: number) {
        this.__shared.set(newValue);
    }
    private __theme: ObservedPropertyAbstractPU<string>;
    get theme() {
        return this.__theme.get();
    }
    set theme(newValue: string) {
        this.__theme.set(newValue);
    }
    private __task: SynchedPropertyNesedObjectPU<Task>;
    get task() {
        return this.__task.get();
    }
    private __storageLinked: ObservedPropertyAbstractPU<number>;
    get storageLinked() {
        return this.__storageLinked.get();
    }
    set storageLinked(newValue: number) {
        this.__storageLinked.set(newValue);
    }
    private __storagePropped: ObservedPropertyAbstractPU<number>;
    get storagePropped() {
        return this.__storagePropped.get();
    }
    set storagePropped(newValue: number) {
        this.__storagePropped.set(newValue);
    }
    private __localStorageLinked: ObservedPropertyAbstractPU<number> = this.createLocalStorageLink<number>('localKey', 0, "localStorageLinked");
    get localStorageLinked() {
        return this.__localStorageLinked.get();
    }
    set localStorageLinked(newValue: number) {
        this.__localStorageLinked.set(newValue);
    }
    private __localStoragePropped: ObservedPropertyAbstractPU<number> = this.createLocalStorageProp<number>('localKey', 0, "localStoragePropped");
    get localStoragePropped() {
        return this.__localStoragePropped.get();
    }
    set localStoragePropped(newValue: number) {
        this.__localStoragePropped.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(this.theme);
        }, Text);
        Text.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
registerNamedRoute(() => new StateHost(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
