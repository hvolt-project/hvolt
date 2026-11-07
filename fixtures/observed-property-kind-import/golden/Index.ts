if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ObservedPropertyKindImport_Params {
    importedEnum?: ImportedKind;
    importedAlias?: ImportedAlias;
    importedAliasUnion?: ImportedAlias | undefined;
}
import { ImportedKind } from "@normalized:N&&&entry/src/main/ets/common/Kinds&";
import type { ImportedAlias } from "@normalized:N&&&entry/src/main/ets/common/Kinds&";
class ObservedPropertyKindImport extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__importedEnum = new ObservedPropertySimplePU(ImportedKind.A, this, "importedEnum");
        this.__importedAlias = new ObservedPropertySimplePU('x', this, "importedAlias");
        this.__importedAliasUnion = new ObservedPropertySimplePU(undefined, this, "importedAliasUnion");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ObservedPropertyKindImport_Params) {
        if (params.importedEnum !== undefined) {
            this.importedEnum = params.importedEnum;
        }
        if (params.importedAlias !== undefined) {
            this.importedAlias = params.importedAlias;
        }
        if (params.importedAliasUnion !== undefined) {
            this.importedAliasUnion = params.importedAliasUnion;
        }
    }
    updateStateVars(params: ObservedPropertyKindImport_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__importedEnum.purgeDependencyOnElmtId(rmElmtId);
        this.__importedAlias.purgeDependencyOnElmtId(rmElmtId);
        this.__importedAliasUnion.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__importedEnum.aboutToBeDeleted();
        this.__importedAlias.aboutToBeDeleted();
        this.__importedAliasUnion.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __importedEnum: ObservedPropertySimplePU<ImportedKind>;
    get importedEnum() {
        return this.__importedEnum.get();
    }
    set importedEnum(newValue: ImportedKind) {
        this.__importedEnum.set(newValue);
    }
    private __importedAlias: ObservedPropertySimplePU<ImportedAlias>;
    get importedAlias() {
        return this.__importedAlias.get();
    }
    set importedAlias(newValue: ImportedAlias) {
        this.__importedAlias.set(newValue);
    }
    private __importedAliasUnion: ObservedPropertySimplePU<ImportedAlias | undefined>;
    get importedAliasUnion() {
        return this.__importedAliasUnion.get();
    }
    set importedAliasUnion(newValue: ImportedAlias | undefined) {
        this.__importedAliasUnion.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(`${this.importedEnum}${this.importedAlias}`);
        }, Text);
        Text.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "ObservedPropertyKindImport";
    }
}
registerNamedRoute(() => new ObservedPropertyKindImport(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
