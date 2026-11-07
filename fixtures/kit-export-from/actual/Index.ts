if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface Index_Params {}
import curves from '@native:ohos.curves';
export { default as curves } from "@native:ohos.curves";
export * from "@ohos:ability.ability";
export * from "@ohos:ability.errorCode";
export * from "@ohos:ability.featureAbility";
export * from "@ohos:ability.particleAbility";
export * from "@ohos:abilityAccessCtrl";
export * from "@ohos:app.ability.Ability";
export * from "@ohos:app.ability.AbilityConstant";
export * from "@ohos:app.ability.AbilityLifecycleCallback";
export * from "@ohos:app.ability.abilityManager";
export * from "@ohos:app.ability.AbilityStage";
export * from "@ohos:app.ability.ActionExtensionAbility";
export * from "@ohos:app.ability.ApplicationStateChangeCallback";
export * from "@ohos:app.ability.appManager";
export * from "@ohos:app.ability.appRecovery";
export * from "@ohos:app.ability.autoFillManager";
export * from "@ohos:app.ability.autoStartupManager";
export * from "@ohos:app.ability.ChildProcess";
export * from "@ohos:app.ability.childProcessManager";
export * from "@ohos:app.ability.ChildProcessArgs";
export * from "@ohos:app.ability.ChildProcessOptions";
export * from "@ohos:app.ability.common";
export * from "@ohos:app.ability.Configuration";
export * from "@ohos:app.ability.ConfigurationConstant";
export * from "@ohos:app.ability.contextConstant";
export * from "@ohos:app.ability.dataUriUtils";
export * from "@ohos:app.ability.dialogRequest";
export * from "@ohos:app.ability.EnvironmentCallback";
export * from "@ohos:app.ability.errorManager";
export * from "@ohos:app.ability.ExtensionAbility";
export * from "@ohos:app.ability.insightIntent";
export * from "@ohos:app.ability.InsightIntentContext";
export * from "@ohos:app.ability.insightIntentProvider";
export * from "@ohos:app.ability.InsightIntentExecutor";
export * from "@ohos:app.ability.InsightIntentDecorator";
export * from "@ohos:app.ability.InsightIntentEntryExecutor";
export * from "@ohos:app.ability.OpenLinkOptions";
export * from "@ohos:app.ability.ShareExtensionAbility";
export * from "@ohos:app.ability.StartOptions";
export * from "@ohos:app.ability.UIAbility";
export * from "@ohos:app.ability.UIExtensionAbility";
export * from "@ohos:app.ability.UIExtensionContentSession";
export * from "@ohos:app.ability.Want";
export * from "@ohos:app.ability.wantConstant";
export * from "@ohos:app.ability.wantAgent";
export * from "@ohos:application.uriPermissionManager";
export * from "@ohos:bundle.bundleManager";
export * from "@ohos:bundle";
export * from "@ohos:bundle.defaultAppManager";
export * from "@ohos:bundle.launcherBundleManager";
export * from "@ohos:bundle.overlay";
export * from "@ohos:continuation.continuationManager";
export * from "@ohos:app.ability.continueManager";
export * from "@ohos:package";
export * from "@ohos:privacyManager";
export * from "@ohos:app.ability.EmbeddedUIExtensionAbility";
export * from "@ohos:app.appstartup.StartupConfig";
export * from "@ohos:app.appstartup.StartupConfigEntry";
export * from "@ohos:app.appstartup.StartupListener";
export * from "@ohos:app.appstartup.StartupTask";
export * from "@ohos:app.appstartup.startupManager";
export * from "@ohos:app.ability.sendableContextManager";
export * from "@ohos:ability.screenLockFileManager";
export * from "@ohos:app.ability.AtomicServiceOptions";
export * from "@ohos:app.ability.EmbeddableUIAbility";
export * from "@ohos:app.ability.PhotoEditorExtensionAbility";
export * from "@ohos:bundle.shortcutManager";
export * from "@ohos:bundle.skillManager";
export * from "@ohos:app.ability.scriptManager";
export * from "@ohos:app.ability.application";
export * from "@ohos:app.ability.CompletionHandler";
export * from "@ohos:app.ability.AppServiceExtensionAbility";
export * from "@ohos:app.ability.kioskManager";
export * from "@ohos:app.ability.CompletionHandlerForAtomicService";
export * from "@ohos:app.ability.CompletionHandlerForAbilityStartCallback";
export * from "@ohos:app.ability.systemConfiguration";
export * from "@ohos:app.agent.AgentExtensionAbility";
export * from "@ohos:app.agent.AgentUIExtensionAbility";
export * from "@ohos:app.agent.agentConstant";
export * from "@ohos:app.ability.hyperSnapManager";
class Index extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: Index_Params) {}
	updateStateVars(params: Index_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Text.create(`${curves.cubicBezierCurve(.1, .2, .3, .4).interpolate(.5)}`);
		}, Text);
		Text.pop();
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
