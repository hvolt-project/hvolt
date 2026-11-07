if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface MapTypeArgs_Params {
    profiles?: Map<string, string>;
    holder?: MapHolder;
}
import { MapHolder } from "@normalized:N&&&entry/src/main/ets/common/MapHolder&";
class MapTypeArgs extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__profiles = this.createStorageLink('mapProfiles', new Map<string, string>([]), "profiles");
        this.__holder = new ObservedPropertyObjectPU(new MapHolder(), this, "holder");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: MapTypeArgs_Params) {
        if (params.holder !== undefined) {
            this.holder = params.holder;
        }
    }
    updateStateVars(params: MapTypeArgs_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__profiles.purgeDependencyOnElmtId(rmElmtId);
        this.__holder.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__profiles.aboutToBeDeleted();
        this.__holder.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __profiles: ObservedPropertyAbstractPU<Map<string, string>>;
    get profiles() {
        return this.__profiles.get();
    }
    set profiles(newValue: Map<string, string>) {
        this.__profiles.set(newValue);
    }
    private __holder: ObservedPropertyObjectPU<MapHolder>;
    get holder() {
        return this.__holder.get();
    }
    set holder(newValue: MapHolder) {
        this.__holder.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.profiles.size + this.holder.total()}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "MapTypeArgs";
    }
}
registerNamedRoute(() => new MapTypeArgs(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
