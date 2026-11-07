if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface SameNameWithResource_Params {}
interface Info {
	label: string;
	icon: Resource;
}
class SameNameWithResource extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: SameNameWithResource_Params) {
    }
    updateStateVars(params: SameNameWithResource_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
describe(): string {
    const label = 'x'
    const info: Info = { label: label, icon: { "id": 16777216, "type": 10003, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' } }
    return info.label
  }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                Text.create(`${this.describe()}`);
            }, Text);
            Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "SameNameWithResource";
    }
}
registerNamedRoute(() => new SameNameWithResource(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
