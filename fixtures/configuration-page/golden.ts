if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ConfigurationPage_Params {
    favoriteProfiles?: Map<string, string>;
    appConfig?: AppConfig;
    uiConfig?: UIConfig;
    configStr?: string;
    stackAlignContentAlignment?: Alignment;
    configAddPageInfos?: NavPathStack;
    icon_emphasize?: ResourceColor;
    bottomRectHeight?: number;
    mainWindowId?: number;
    isON?: isON;
    isForward?: boolean;
    windowMode?: boolean;
    isNewConfig?: boolean;
    isRefreshing?: boolean;
    isShowMenu?: boolean;
    isFromFile?: boolean;
    isLoading?: boolean;
    showSort?: boolean;
    isShowConfigAddBindSheet?: boolean;
    isConfigDownloading?: boolean;
    isShowQRCode?: boolean;
    isEnableIndexForegroundBlur?: boolean;
    isSmallSquareScreen?: boolean;
    isPXSecScreen?: boolean;
    isClearData?: boolean;
    isRolling?: boolean;
    heightBp?: HeightBreakpoint;
    widthBp?: WidthBreakpoint;
    paddingButtom?: number | Resource;
    tabCurrentPage?: number;
    curOffsetRemain?: number;
    heightTop?: number;
    curYOffset?: number;
    currentTouchConfigData?: ConfigData;
    configList?: Array<ConfigData>;
    currentRadioConfigData?: ConfigData;
    // ContextMenu 列表项 动态替换用
    configListLongPressCtxMenuForUnFavorite?: LongPressContextMenuItem;
    configListLongPressCtxMenuForFavorite?: LongPressContextMenuItem;
    /* 自定义弹框 */
    // 配置是否保存弹框 Controller定义
    configCustomDialogController?: CustomDialogController | null;
}
interface CustomConfirmDialog_Params {
    controller?: CustomDialogController;
    title?: ResourceStr;
    cancelText?: ResourceStr;
    confirmText?: ResourceStr;
    cancelButtonBackgroundColor?: ResourceColor;
    confirmButtonBackgroundColor?: ResourceColor;
    cancel?: () => void;
    confirm?: () => void;
}
interface QRCodeDialog_Params {
    theDialogTitle?: ResourceStr;
    QRCodeParam?: QRCodeParam;
    confirm?: () => void;
    cancel?: () => void;
}
interface ImportConfigFromFile_Params {
    configData?: ConfigData;
}
interface ImportConfigFromURL_Params {
    configData?: ConfigData;
}
interface ConfigAdd_Params {
}
interface RoundButton_Params {
    Icon2?: ResourceStr;
}
interface TopBar_Params {
    title?: ResourceStr;
    titleFontSize?: number;
    icons?: ResourceStr[];
    iconSize?: number;
    onFClick?: () => void;
    onSClick?: () => void;
}
interface Nothing_Params {
    name?: ResourceStr;
}
interface CellularData_Params {
    info?: SubscriptionInfo;
}
import { LengthMetrics } from "@ohos:arkui.node";
import { SymbolGlyphModifier } from "@ohos:arkui.modifier";
import hilog from "@ohos:hilog";
import type { Callback } from "@ohos:base";
import type harmonyShare from "@hms:collaboration.harmonyShare";
import utd from "@ohos:data.uniformTypeDescriptor";
import type common from "@ohos:app.ability.common";
import type { Context } from "@ohos:abilityAccessCtrl";
import type { BusinessError } from "@ohos:base";
import fileUri from "@ohos:file.fileuri";
import fs from "@ohos:file.fs";
import picker from "@ohos:file.picker";
import JSON from "@ohos:util.json";
// --- stubs for the project-local imports (see README.md) ---
// ../common/entity/Constants
const TAB_CONTENT_TITLE_FONT_SIZE: number = 16;
const TAB_CONTENT_TITLE_HEIGHT: number = 56;
const TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE: string = '16fp';
const LIST_SPACE: number = 10;
const BORDER_RADIUS_20: number = 20;
const BIND_SHEET_CONTAINER_HEIGHT: number = 300;
const LIST_ITEM_PADDING: Padding = { left: 14, right: 14, top: 5, bottom: 5 };
const CONTENT_END_OFFSET_150: number = 150;
const APP_LINKING: string = 'clashbox://share';
const BIND_SHEET_DRAG_BAR_HEIGHT: number = 20;
const BIND_SHEET_TITLE_CONTAINER_HEIGHT: number = 40;
// ../components/Common/Common
const icon_tertiary: ResourceColor = '#99000000';
// proxy_core
enum ProfileType {
    Url = 0,
    File = 1,
    Manual = 2
}
// ../common/entity/PageArgumentEntity
class SubscriptionInfo {
    upload: number = 0;
    download: number = 0;
    total: number = 0;
}
class ConfigData {
    configId: string = '';
    configName: string = '';
    configUrl: string = '';
    updateTime: string = '';
    lastUpdateDateTime: number = 0;
    sourceType: ProfileType = ProfileType.Url;
    disableUpdate: boolean = false;
    isFavorite: boolean = false;
    isNewConfig: boolean = false;
    subscriptionInfo?: SubscriptionInfo = undefined;
}
const NewConfigData: ConfigData = new ConfigData();
// ../entryability/AppState
class AppConfig {
    currentProfileId: string | null = null;
    currentProfileName: string | undefined = undefined;
    currentProxyName: string | undefined = undefined;
}
class UIConfig {
    ShowConfigPop: boolean = false;
    isVibrate: boolean = true;
    isAnimation: boolean = true;
    isBlurr: boolean = true;
    isFloatButtonVisible: boolean = true;
    tabStyle: string = 'custom';
    buttonPositionIndex: number = 0;
    animationSpeed: number = 300;
    isFirstImportConfig: boolean = false;
}
// ../common/EventHub
enum EventKey {
    AddConfig = "AddConfig",
    LoadConfigStr = "LoadConfigStr",
    FetchProfile = "FetchProfile"
}
class EventHub {
    static on(key: EventKey, callback: () => void): void {
    }
    static off(key: EventKey): void {
    }
    static sendEvent(key: EventKey): void {
    }
}
// ../common/utils/ResourceStringUtil, ../common/utils/TimeConvertUtil
function getResourceString(res: Resource, parent: Object): string {
    return '';
}
function convertTime(seconds: number): string {
    return '';
}
// ../common/breakpoint/BreakPoint
function ListGutter(widthBp: WidthBreakpoint, heightBp: HeightBreakpoint): Dimension {
    return 0;
}
function ListLanes(widthBp: WidthBreakpoint, heightBp: HeightBreakpoint): number {
    return 1;
}
function NavigationListPadding(widthBp: WidthBreakpoint, heightBp: HeightBreakpoint): number {
    return 0;
}
function TabContentTitleMargin(widthBp: WidthBreakpoint, heightBp: HeightBreakpoint): number {
    return 0;
}
// ../common/utils/Animation
class isON {
    toggleAnim: boolean = false;
}
class customAnimationUtil {
    static isOpacity(config: UIConfig): TransitionEffect {
        return TransitionEffect.OPACITY;
    }
    static isClickEffect(config: UIConfig): ClickEffect {
        return { level: ClickEffectLevel.LIGHT };
    }
    static isScaleTran(index: number, config: UIConfig): TransitionEffect {
        return TransitionEffect.OPACITY;
    }
    static getFloatButtonTransition(config: UIConfig, position: number): TransitionEffect {
        return TransitionEffect.OPACITY;
    }
}
// ../common/utils/VibratorUtil
class customVibrator {
    static vibratorTriggerOfHapticClockTimer(): void {
    }
}
// ../entryability/ClashViewModel
class SocketProxy {
    async reset(): Promise<void> {
    }
}
class ClashViewModel {
    static socketProxy: SocketProxy = new SocketProxy();
    static async updateProfileConfig(configId: string | null): Promise<void> {
    }
    static async loadConfig(force: boolean, reason: string): Promise<void> {
    }
    static async deleteProfile(configId: string): Promise<void> {
    }
}
// xb_components
interface ShareData {
    utd: utd.UniformDataType;
    content: string;
    thumbnailUri: string;
    title: ResourceStr;
    description: string;
    label: string;
    uri: string;
}
interface ToastOptions {
    message: ResourceStr;
    duration?: number;
}
class Xb_GetResourceStringHolder {
}
function Xb_GetResourceString(res: Resource, parent: Object): string {
    return '';
}
function Xb_TabBarVertical(widthBp: WidthBreakpoint, heightBp: HeightBreakpoint): boolean {
    return false;
}
class Xb_Share {
    static shareData: ShareData | undefined = undefined;
    static inputInfo(context: UIContext): void {
    }
    static async sharePlant(context: common.UIAbilityContext): Promise<void> {
    }
    static immersiveListening(type: string): void {
    }
    static immersiveDisablingListening(type: string): void {
    }
}
class Xb_PreferenceUtil {
    static putSync(key: string, value: string): void {
    }
    static async save(): Promise<void> {
    }
}
class Xb_ToastUtil {
    static showToast(options: ToastOptions): void {
    }
}
// ../components/Start/Popup
const PopTips: string[] = ['', '', ''];
function PopupBuilder(ctx: object, tip: string, onClose: () => void, parent = null) {
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
        Column.create();
    }, Column);
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
        Text.create(tip);
    }, Text);
    Text.pop();
    Column.pop();
}
class CellularData extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__info = new SynchedPropertyObjectOneWayPU(params.info, this, "info");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: CellularData_Params) {
    }
    updateStateVars(params: CellularData_Params) {
        this.__info.reset(params.info);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__info.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__info.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __info: SynchedPropertySimpleOneWayPU<SubscriptionInfo>;
    get info() {
        return this.__info.get();
    }
    set info(newValue: SubscriptionInfo) {
        this.__info.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
        }, Row);
        Row.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class Nothing extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__name = new SynchedPropertyObjectOneWayPU(params.name, this, "name");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: Nothing_Params) {
    }
    updateStateVars(params: Nothing_Params) {
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
    private __name: SynchedPropertySimpleOneWayPU<ResourceStr>;
    get name() {
        return this.__name.get();
    }
    set name(newValue: ResourceStr) {
        this.__name.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
        }, Row);
        Row.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class TopBar extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__title = new SynchedPropertyObjectOneWayPU(params.title, this, "title");
        this.__titleFontSize = new SynchedPropertySimpleOneWayPU(params.titleFontSize, this, "titleFontSize");
        this.__icons = new SynchedPropertyObjectOneWayPU(params.icons, this, "icons");
        this.__iconSize = new SynchedPropertySimpleOneWayPU(params.iconSize, this, "iconSize");
        this.onFClick = () => {
        };
        this.onSClick = () => {
        };
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: TopBar_Params) {
        if (params.onFClick !== undefined) {
            this.onFClick = params.onFClick;
        }
        if (params.onSClick !== undefined) {
            this.onSClick = params.onSClick;
        }
    }
    updateStateVars(params: TopBar_Params) {
        this.__title.reset(params.title);
        this.__titleFontSize.reset(params.titleFontSize);
        this.__icons.reset(params.icons);
        this.__iconSize.reset(params.iconSize);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__title.purgeDependencyOnElmtId(rmElmtId);
        this.__titleFontSize.purgeDependencyOnElmtId(rmElmtId);
        this.__icons.purgeDependencyOnElmtId(rmElmtId);
        this.__iconSize.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__title.aboutToBeDeleted();
        this.__titleFontSize.aboutToBeDeleted();
        this.__icons.aboutToBeDeleted();
        this.__iconSize.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __title: SynchedPropertySimpleOneWayPU<ResourceStr>;
    get title() {
        return this.__title.get();
    }
    set title(newValue: ResourceStr) {
        this.__title.set(newValue);
    }
    private __titleFontSize: SynchedPropertySimpleOneWayPU<number>;
    get titleFontSize() {
        return this.__titleFontSize.get();
    }
    set titleFontSize(newValue: number) {
        this.__titleFontSize.set(newValue);
    }
    private __icons: SynchedPropertySimpleOneWayPU<ResourceStr[]>;
    get icons() {
        return this.__icons.get();
    }
    set icons(newValue: ResourceStr[]) {
        this.__icons.set(newValue);
    }
    private __iconSize: SynchedPropertySimpleOneWayPU<number>;
    get iconSize() {
        return this.__iconSize.get();
    }
    set iconSize(newValue: number) {
        this.__iconSize.set(newValue);
    }
    private onFClick: () => void;
    private onSClick: () => void;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
        }, Row);
        Row.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class RoundButton extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__Icon2 = new SynchedPropertyObjectOneWayPU(params.Icon2, this, "Icon2");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: RoundButton_Params) {
    }
    updateStateVars(params: RoundButton_Params) {
        this.__Icon2.reset(params.Icon2);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__Icon2.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__Icon2.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __Icon2: SynchedPropertySimpleOneWayPU<ResourceStr>;
    get Icon2() {
        return this.__Icon2.get();
    }
    set Icon2(newValue: ResourceStr) {
        this.__Icon2.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
        }, Row);
        Row.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class ConfigAdd extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ConfigAdd_Params) {
    }
    updateStateVars(params: ConfigAdd_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
        }, Row);
        Row.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class ImportConfigFromURL extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__configData = new SynchedPropertyObjectOneWayPU(params.configData, this, "configData");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ImportConfigFromURL_Params) {
    }
    updateStateVars(params: ImportConfigFromURL_Params) {
        this.__configData.reset(params.configData);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__configData.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__configData.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __configData: SynchedPropertySimpleOneWayPU<ConfigData>;
    get configData() {
        return this.__configData.get();
    }
    set configData(newValue: ConfigData) {
        this.__configData.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
        }, Row);
        Row.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class ImportConfigFromFile extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__configData = new SynchedPropertyObjectOneWayPU(params.configData, this, "configData");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ImportConfigFromFile_Params) {
    }
    updateStateVars(params: ImportConfigFromFile_Params) {
        this.__configData.reset(params.configData);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__configData.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__configData.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __configData: SynchedPropertySimpleOneWayPU<ConfigData>;
    get configData() {
        return this.__configData.get();
    }
    set configData(newValue: ConfigData) {
        this.__configData.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
        }, Row);
        Row.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
interface QRCodeParam {
    url: string;
    name: string;
}
class QRCodeDialog extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__theDialogTitle = new SynchedPropertyObjectOneWayPU(params.theDialogTitle, this, "theDialogTitle");
        this.__QRCodeParam = new SynchedPropertyObjectOneWayPU(params.QRCodeParam, this, "QRCodeParam");
        this.confirm = () => {
        };
        this.cancel = () => {
        };
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: QRCodeDialog_Params) {
        if (params.confirm !== undefined) {
            this.confirm = params.confirm;
        }
        if (params.cancel !== undefined) {
            this.cancel = params.cancel;
        }
    }
    updateStateVars(params: QRCodeDialog_Params) {
        this.__theDialogTitle.reset(params.theDialogTitle);
        this.__QRCodeParam.reset(params.QRCodeParam);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__theDialogTitle.purgeDependencyOnElmtId(rmElmtId);
        this.__QRCodeParam.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__theDialogTitle.aboutToBeDeleted();
        this.__QRCodeParam.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __theDialogTitle: SynchedPropertySimpleOneWayPU<ResourceStr>;
    get theDialogTitle() {
        return this.__theDialogTitle.get();
    }
    set theDialogTitle(newValue: ResourceStr) {
        this.__theDialogTitle.set(newValue);
    }
    private __QRCodeParam: SynchedPropertySimpleOneWayPU<QRCodeParam>;
    get QRCodeParam() {
        return this.__QRCodeParam.get();
    }
    set QRCodeParam(newValue: QRCodeParam) {
        this.__QRCodeParam.set(newValue);
    }
    private confirm: () => void;
    private cancel: () => void;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
        }, Row);
        Row.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class CustomConfirmDialog extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.controller = undefined;
        this.__title = new SynchedPropertyObjectOneWayPU(params.title, this, "title");
        this.__cancelText = new SynchedPropertyObjectOneWayPU(params.cancelText, this, "cancelText");
        this.__confirmText = new SynchedPropertyObjectOneWayPU(params.confirmText, this, "confirmText");
        this.__cancelButtonBackgroundColor = new SynchedPropertyObjectOneWayPU(params.cancelButtonBackgroundColor, this, "cancelButtonBackgroundColor");
        this.__confirmButtonBackgroundColor = new SynchedPropertyObjectOneWayPU(params.confirmButtonBackgroundColor, this, "confirmButtonBackgroundColor");
        this.cancel = () => {
        };
        this.confirm = () => {
        };
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: CustomConfirmDialog_Params) {
        if (params.controller !== undefined) {
            this.controller = params.controller;
        }
        if (params.cancel !== undefined) {
            this.cancel = params.cancel;
        }
        if (params.confirm !== undefined) {
            this.confirm = params.confirm;
        }
    }
    updateStateVars(params: CustomConfirmDialog_Params) {
        this.__title.reset(params.title);
        this.__cancelText.reset(params.cancelText);
        this.__confirmText.reset(params.confirmText);
        this.__cancelButtonBackgroundColor.reset(params.cancelButtonBackgroundColor);
        this.__confirmButtonBackgroundColor.reset(params.confirmButtonBackgroundColor);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__title.purgeDependencyOnElmtId(rmElmtId);
        this.__cancelText.purgeDependencyOnElmtId(rmElmtId);
        this.__confirmText.purgeDependencyOnElmtId(rmElmtId);
        this.__cancelButtonBackgroundColor.purgeDependencyOnElmtId(rmElmtId);
        this.__confirmButtonBackgroundColor.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__title.aboutToBeDeleted();
        this.__cancelText.aboutToBeDeleted();
        this.__confirmText.aboutToBeDeleted();
        this.__cancelButtonBackgroundColor.aboutToBeDeleted();
        this.__confirmButtonBackgroundColor.aboutToBeDeleted();
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
    private __cancelText: SynchedPropertySimpleOneWayPU<ResourceStr>;
    get cancelText() {
        return this.__cancelText.get();
    }
    set cancelText(newValue: ResourceStr) {
        this.__cancelText.set(newValue);
    }
    private __confirmText: SynchedPropertySimpleOneWayPU<ResourceStr>;
    get confirmText() {
        return this.__confirmText.get();
    }
    set confirmText(newValue: ResourceStr) {
        this.__confirmText.set(newValue);
    }
    private __cancelButtonBackgroundColor: SynchedPropertySimpleOneWayPU<ResourceColor>;
    get cancelButtonBackgroundColor() {
        return this.__cancelButtonBackgroundColor.get();
    }
    set cancelButtonBackgroundColor(newValue: ResourceColor) {
        this.__cancelButtonBackgroundColor.set(newValue);
    }
    private __confirmButtonBackgroundColor: SynchedPropertySimpleOneWayPU<ResourceColor>;
    get confirmButtonBackgroundColor() {
        return this.__confirmButtonBackgroundColor.get();
    }
    set confirmButtonBackgroundColor(newValue: ResourceColor) {
        this.__confirmButtonBackgroundColor.set(newValue);
    }
    private cancel: () => void;
    private confirm: () => void;
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
// --- original file follows verbatim (its import block is lines 1-46 of the source) ---
/**
 * 长按菜单参数
 */
interface LongPressContextMenuItem {
    symbolStartIcon?: SymbolGlyphModifier;
    symbolEndIcon?: SymbolGlyphModifier;
    content: ResourceStr;
    callback: Callback<void>;
}
const componentName: string = 'ConfigurationPage';
class ConfigurationPage extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__favoriteProfiles = this.createStorageLink('favoriteProfiles', new Map<string, string>([]), "favoriteProfiles");
        this.__appConfig = this.createStorageLink('appConfig', new AppConfig(), "appConfig");
        this.__uiConfig = this.createStorageLink('uiConfig', new UIConfig(), "uiConfig");
        this.__configStr = this.createStorageLink('ConfigContent', ''
        // 所有子组件在容器内的对齐方式
        , "configStr");
        this.__stackAlignContentAlignment = this.initializeConsume('stackAlignContentAlignment', "stackAlignContentAlignment");
        this.__configAddPageInfos = this.initializeConsume('configAddPageInfos', "configAddPageInfos");
        this.__icon_emphasize = this.initializeConsume('icon_emphasize', "icon_emphasize");
        this.__bottomRectHeight = this.createStorageProp('bottomRectHeight', 0
        // 主窗口ID
        , "bottomRectHeight");
        this.__mainWindowId = this.createStorageProp('MainWindowId', 1
        // 开关不触发list转场动画判定
        , "mainWindowId");
        this.__isON = new ObservedPropertyObjectPU(new isON()
        // 前台判断变量
        , this, "isON");
        this.__isForward = this.createStorageProp('ForwardState', false
        // 窗口模式
        , "isForward");
        this.__windowMode = this.createStorageProp('WindowMode', false
        // 是否为新建配置
        , "windowMode");
        this.__isNewConfig = this.createStorageLink('ConfigNew', false, "isNewConfig");
        this.__isRefreshing = new ObservedPropertySimplePU(false, this, "isRefreshing");
        this.__isShowMenu = new ObservedPropertySimplePU(false
        // 是否来自文件导入
        , this, "isShowMenu");
        this.__isFromFile = new ObservedPropertySimplePU(false
        // loading动画
        , this, "isFromFile");
        this.__isLoading = new ObservedPropertySimplePU(false
        // 展示排序界面
        , this, "isLoading");
        this.__showSort = new ObservedPropertySimplePU(false
        // 是否显示bindSheet
        , this, "showSort");
        this.__isShowConfigAddBindSheet = this.createStorageLink('ShowConfigBindSheet', false, "isShowConfigAddBindSheet");
        this.__isConfigDownloading = this.createStorageLink('ConfigDownloading', false
        // 是否显示二维码半模态
        , "isConfigDownloading");
        this.__isShowQRCode = new ObservedPropertySimplePU(false
        /* 是否开启 `Index` 页面的前景模糊 */
        , this, "isShowQRCode");
        this.__isEnableIndexForegroundBlur = this.initializeConsume('isEnableIndexForegroundBlur', "isEnableIndexForegroundBlur");
        this.__isSmallSquareScreen = this.initializeConsume("isSmallSquareScreen", "isSmallSquareScreen");
        this.__isPXSecScreen = this.initializeConsume("isPXSecScreen", "isPXSecScreen");
        this.__isClearData = this.initializeConsume("isClearData", "isClearData");
        this.__isRolling = this.initializeConsume("isRolling", "isRolling");
        this.__heightBp = this.createStorageLink('currentHeightBreakpoint', 0
        // 宽度断点
        , "heightBp");
        this.__widthBp = this.createStorageLink('currentWidthBreakpoint', 1, "widthBp");
        this.__paddingButtom = this.initializeConsume("paddingButtom", "paddingButtom");
        this.__tabCurrentPage = this.initializeConsume("tabCurrentPage", "tabCurrentPage");
        this.__curOffsetRemain = this.initializeConsume("curOffsetRemain", "curOffsetRemain");
        this.__heightTop = this.initializeConsume("heightTop", "heightTop");
        this.__curYOffset = this.initializeConsume("curYOffset", "curYOffset");
        this.__currentTouchConfigData = this.createStorageLink('currentTouchConfigData', NewConfigData, "currentTouchConfigData");
        this.__configList = this.initializeConsume('Configuration', "configList");
        this.__currentRadioConfigData = new ObservedPropertyObjectPU(NewConfigData, this, "currentRadioConfigData");
        this.configListLongPressCtxMenuForUnFavorite = {
            symbolStartIcon: new SymbolGlyphModifier({ "id": 125831521, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" })
                .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE).fontColor([{ "id": 125831041, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }]),
            content: { "id": 16777242, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, callback: () => { this.cancelFavoriteConfig(); }
        };
        this.configListLongPressCtxMenuForFavorite = {
            symbolStartIcon: new SymbolGlyphModifier({ "id": 125831520, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" })
                .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE).fontColor([{ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }]),
            content: { "id": 16777221, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, callback: () => { this.favoriteConfig(); }
        };
        this.configCustomDialogController = new CustomDialogController({
            builder: () => {
                let jsDialog = new CustomConfirmDialog(this, {
                    title: { "id": 16777232, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                    cancelText: { "id": 16777231, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                    confirmText: { "id": 16777220, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                    cancelButtonBackgroundColor: Color.Transparent,
                    // 不可以使用`@Consume`注解的变量
                    // confirmButtonBackgroundColor: this.icon_emphasize,
                    cancel: () => {
                        this.isEnableIndexForegroundBlur = false;
                        this.deleteConfig();
                    },
                    confirm: () => {
                        this.isEnableIndexForegroundBlur = false;
                        Xb_ToastUtil.showToast({ message: { "id": 16777227, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
                    }
                }, undefined, -1, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 727, col: 14 });
                jsDialog.setController(this.
                /* 自定义弹框 */
                // 配置是否保存弹框 Controller定义
                configCustomDialogController);
                ViewPU.create(jsDialog);
                let paramsLambda = () => {
                    return {
                        title: { "id": 16777232, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                        cancelText: { "id": 16777231, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                        confirmText: { "id": 16777220, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                        cancelButtonBackgroundColor: Color.Transparent,
                        // 不可以使用`@Consume`注解的变量
                        // confirmButtonBackgroundColor: this.icon_emphasize,
                        cancel: () => {
                            this.isEnableIndexForegroundBlur = false;
                            this.deleteConfig();
                        },
                        confirm: () => {
                            this.isEnableIndexForegroundBlur = false;
                            Xb_ToastUtil.showToast({ message: { "id": 16777227, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
                        }
                    };
                };
                jsDialog.paramsGenerator_ = paramsLambda;
            },
            cancel: () => {
                this.isEnableIndexForegroundBlur = false;
            },
            autoCancel: true,
            alignment: DialogAlignment.Center,
            width: 328,
            backgroundColor: { "id": 16777251, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }
        }, this);
        this.setInitiallyProvidedValue(params);
        this.declareWatch("appConfig", this.onChangeConfig);
        this.declareWatch("isForward", this.onForwardChange);
        this.declareWatch("tabCurrentPage", this.onForwardChange);
        this.declareWatch("configList", this.onConfigListChange);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ConfigurationPage_Params) {
        if (params.isON !== undefined) {
            this.isON = params.isON;
        }
        if (params.isRefreshing !== undefined) {
            this.isRefreshing = params.isRefreshing;
        }
        if (params.isShowMenu !== undefined) {
            this.isShowMenu = params.isShowMenu;
        }
        if (params.isFromFile !== undefined) {
            this.isFromFile = params.isFromFile;
        }
        if (params.isLoading !== undefined) {
            this.isLoading = params.isLoading;
        }
        if (params.showSort !== undefined) {
            this.showSort = params.showSort;
        }
        if (params.isShowQRCode !== undefined) {
            this.isShowQRCode = params.isShowQRCode;
        }
        if (params.currentRadioConfigData !== undefined) {
            this.currentRadioConfigData = params.currentRadioConfigData;
        }
        if (params.configListLongPressCtxMenuForUnFavorite !== undefined) {
            this.configListLongPressCtxMenuForUnFavorite = params.configListLongPressCtxMenuForUnFavorite;
        }
        if (params.configListLongPressCtxMenuForFavorite !== undefined) {
            this.configListLongPressCtxMenuForFavorite = params.configListLongPressCtxMenuForFavorite;
        }
        if (params.configCustomDialogController !== undefined) {
            this.configCustomDialogController = params.configCustomDialogController;
        }
    }
    updateStateVars(params: ConfigurationPage_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__favoriteProfiles.purgeDependencyOnElmtId(rmElmtId);
        this.__appConfig.purgeDependencyOnElmtId(rmElmtId);
        this.__uiConfig.purgeDependencyOnElmtId(rmElmtId);
        this.__configStr.purgeDependencyOnElmtId(rmElmtId);
        this.__stackAlignContentAlignment.purgeDependencyOnElmtId(rmElmtId);
        this.__configAddPageInfos.purgeDependencyOnElmtId(rmElmtId);
        this.__icon_emphasize.purgeDependencyOnElmtId(rmElmtId);
        this.__bottomRectHeight.purgeDependencyOnElmtId(rmElmtId);
        this.__mainWindowId.purgeDependencyOnElmtId(rmElmtId);
        this.__isON.purgeDependencyOnElmtId(rmElmtId);
        this.__isForward.purgeDependencyOnElmtId(rmElmtId);
        this.__windowMode.purgeDependencyOnElmtId(rmElmtId);
        this.__isNewConfig.purgeDependencyOnElmtId(rmElmtId);
        this.__isRefreshing.purgeDependencyOnElmtId(rmElmtId);
        this.__isShowMenu.purgeDependencyOnElmtId(rmElmtId);
        this.__isFromFile.purgeDependencyOnElmtId(rmElmtId);
        this.__isLoading.purgeDependencyOnElmtId(rmElmtId);
        this.__showSort.purgeDependencyOnElmtId(rmElmtId);
        this.__isShowConfigAddBindSheet.purgeDependencyOnElmtId(rmElmtId);
        this.__isConfigDownloading.purgeDependencyOnElmtId(rmElmtId);
        this.__isShowQRCode.purgeDependencyOnElmtId(rmElmtId);
        this.__isEnableIndexForegroundBlur.purgeDependencyOnElmtId(rmElmtId);
        this.__isSmallSquareScreen.purgeDependencyOnElmtId(rmElmtId);
        this.__isPXSecScreen.purgeDependencyOnElmtId(rmElmtId);
        this.__isClearData.purgeDependencyOnElmtId(rmElmtId);
        this.__isRolling.purgeDependencyOnElmtId(rmElmtId);
        this.__heightBp.purgeDependencyOnElmtId(rmElmtId);
        this.__widthBp.purgeDependencyOnElmtId(rmElmtId);
        this.__paddingButtom.purgeDependencyOnElmtId(rmElmtId);
        this.__tabCurrentPage.purgeDependencyOnElmtId(rmElmtId);
        this.__curOffsetRemain.purgeDependencyOnElmtId(rmElmtId);
        this.__heightTop.purgeDependencyOnElmtId(rmElmtId);
        this.__curYOffset.purgeDependencyOnElmtId(rmElmtId);
        this.__currentTouchConfigData.purgeDependencyOnElmtId(rmElmtId);
        this.__configList.purgeDependencyOnElmtId(rmElmtId);
        this.__currentRadioConfigData.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__favoriteProfiles.aboutToBeDeleted();
        this.__appConfig.aboutToBeDeleted();
        this.__uiConfig.aboutToBeDeleted();
        this.__configStr.aboutToBeDeleted();
        this.__stackAlignContentAlignment.aboutToBeDeleted();
        this.__configAddPageInfos.aboutToBeDeleted();
        this.__icon_emphasize.aboutToBeDeleted();
        this.__bottomRectHeight.aboutToBeDeleted();
        this.__mainWindowId.aboutToBeDeleted();
        this.__isON.aboutToBeDeleted();
        this.__isForward.aboutToBeDeleted();
        this.__windowMode.aboutToBeDeleted();
        this.__isNewConfig.aboutToBeDeleted();
        this.__isRefreshing.aboutToBeDeleted();
        this.__isShowMenu.aboutToBeDeleted();
        this.__isFromFile.aboutToBeDeleted();
        this.__isLoading.aboutToBeDeleted();
        this.__showSort.aboutToBeDeleted();
        this.__isShowConfigAddBindSheet.aboutToBeDeleted();
        this.__isConfigDownloading.aboutToBeDeleted();
        this.__isShowQRCode.aboutToBeDeleted();
        this.__isEnableIndexForegroundBlur.aboutToBeDeleted();
        this.__isSmallSquareScreen.aboutToBeDeleted();
        this.__isPXSecScreen.aboutToBeDeleted();
        this.__isClearData.aboutToBeDeleted();
        this.__isRolling.aboutToBeDeleted();
        this.__heightBp.aboutToBeDeleted();
        this.__widthBp.aboutToBeDeleted();
        this.__paddingButtom.aboutToBeDeleted();
        this.__tabCurrentPage.aboutToBeDeleted();
        this.__curOffsetRemain.aboutToBeDeleted();
        this.__heightTop.aboutToBeDeleted();
        this.__curYOffset.aboutToBeDeleted();
        this.__currentTouchConfigData.aboutToBeDeleted();
        this.__configList.aboutToBeDeleted();
        this.__currentRadioConfigData.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __favoriteProfiles: ObservedPropertyAbstractPU<Map<string, string>>;
    get favoriteProfiles() {
        return this.__favoriteProfiles.get();
    }
    set favoriteProfiles(newValue: Map<string, string>) {
        this.__favoriteProfiles.set(newValue);
    }
    private __appConfig: ObservedPropertyAbstractPU<AppConfig>;
    get appConfig() {
        return this.__appConfig.get();
    }
    set appConfig(newValue: AppConfig) {
        this.__appConfig.set(newValue);
    }
    private __uiConfig: ObservedPropertyAbstractPU<UIConfig>;
    get uiConfig() {
        return this.__uiConfig.get();
    }
    set uiConfig(newValue: UIConfig) {
        this.__uiConfig.set(newValue);
    }
    private __configStr: ObservedPropertyAbstractPU<string>;
    get configStr() {
        return this.__configStr.get();
    }
    set configStr(newValue: string) {
        this.__configStr.set(newValue);
    }
    // 所有子组件在容器内的对齐方式
    private __stackAlignContentAlignment: ObservedPropertyAbstractPU<Alignment
    // 路由栈
    >;
    get stackAlignContentAlignment() {
        return this.__stackAlignContentAlignment.get();
    }
    set stackAlignContentAlignment(newValue: Alignment) {
        this.__stackAlignContentAlignment.set(newValue);
    }
    // 路由栈
    private __configAddPageInfos: ObservedPropertyAbstractPU<NavPathStack
    /* 主题色 */
    >;
    get configAddPageInfos() {
        return this.__configAddPageInfos.get();
    }
    set configAddPageInfos(newValue: NavPathStack) {
        this.__configAddPageInfos.set(newValue);
    }
    /* 主题色 */
    private __icon_emphasize: ObservedPropertyAbstractPU<ResourceColor
    // 导航条区域的高度
    >;
    get icon_emphasize() {
        return this.__icon_emphasize.get();
    }
    set icon_emphasize(newValue: ResourceColor) {
        this.__icon_emphasize.set(newValue);
    }
    // 导航条区域的高度
    private __bottomRectHeight: ObservedPropertyAbstractPU<number>;
    get bottomRectHeight() {
        return this.__bottomRectHeight.get();
    }
    set bottomRectHeight(newValue: number) {
        this.__bottomRectHeight.set(newValue);
    }
    // 主窗口ID
    private __mainWindowId: ObservedPropertyAbstractPU<number>;
    get mainWindowId() {
        return this.__mainWindowId.get();
    }
    set mainWindowId(newValue: number) {
        this.__mainWindowId.set(newValue);
    }
    // 开关不触发list转场动画判定
    private __isON: ObservedPropertyObjectPU<isON>;
    get isON() {
        return this.__isON.get();
    }
    set isON(newValue: isON) {
        this.__isON.set(newValue);
    }
    // 前台判断变量
    private __isForward: ObservedPropertyAbstractPU<boolean>;
    get isForward() {
        return this.__isForward.get();
    }
    set isForward(newValue: boolean) {
        this.__isForward.set(newValue);
    }
    // 窗口模式
    private __windowMode: ObservedPropertyAbstractPU<boolean>;
    get windowMode() {
        return this.__windowMode.get();
    }
    set windowMode(newValue: boolean) {
        this.__windowMode.set(newValue);
    }
    // 是否为新建配置
    private __isNewConfig: ObservedPropertyAbstractPU<boolean>;
    get isNewConfig() {
        return this.__isNewConfig.get();
    }
    set isNewConfig(newValue: boolean) {
        this.__isNewConfig.set(newValue);
    }
    private __isRefreshing: ObservedPropertySimplePU<boolean>;
    get isRefreshing() {
        return this.__isRefreshing.get();
    }
    set isRefreshing(newValue: boolean) {
        this.__isRefreshing.set(newValue);
    }
    private __isShowMenu: ObservedPropertySimplePU<boolean>;
    get isShowMenu() {
        return this.__isShowMenu.get();
    }
    set isShowMenu(newValue: boolean) {
        this.__isShowMenu.set(newValue);
    }
    // 是否来自文件导入
    private __isFromFile: ObservedPropertySimplePU<boolean>;
    get isFromFile() {
        return this.__isFromFile.get();
    }
    set isFromFile(newValue: boolean) {
        this.__isFromFile.set(newValue);
    }
    // loading动画
    private __isLoading: ObservedPropertySimplePU<boolean>;
    get isLoading() {
        return this.__isLoading.get();
    }
    set isLoading(newValue: boolean) {
        this.__isLoading.set(newValue);
    }
    // 展示排序界面
    private __showSort: ObservedPropertySimplePU<boolean>;
    get showSort() {
        return this.__showSort.get();
    }
    set showSort(newValue: boolean) {
        this.__showSort.set(newValue);
    }
    // 是否显示bindSheet
    private __isShowConfigAddBindSheet: ObservedPropertyAbstractPU<boolean>;
    get isShowConfigAddBindSheet() {
        return this.__isShowConfigAddBindSheet.get();
    }
    set isShowConfigAddBindSheet(newValue: boolean) {
        this.__isShowConfigAddBindSheet.set(newValue);
    }
    private __isConfigDownloading: ObservedPropertyAbstractPU<boolean>;
    get isConfigDownloading() {
        return this.__isConfigDownloading.get();
    }
    set isConfigDownloading(newValue: boolean) {
        this.__isConfigDownloading.set(newValue);
    }
    // 是否显示二维码半模态
    private __isShowQRCode: ObservedPropertySimplePU<boolean>;
    get isShowQRCode() {
        return this.__isShowQRCode.get();
    }
    set isShowQRCode(newValue: boolean) {
        this.__isShowQRCode.set(newValue);
    }
    /* 是否开启 `Index` 页面的前景模糊 */
    private __isEnableIndexForegroundBlur: ObservedPropertyAbstractPU<boolean
    // 是否小窗口
    >;
    get isEnableIndexForegroundBlur() {
        return this.__isEnableIndexForegroundBlur.get();
    }
    set isEnableIndexForegroundBlur(newValue: boolean) {
        this.__isEnableIndexForegroundBlur.set(newValue);
    }
    // 是否小窗口
    private __isSmallSquareScreen: ObservedPropertyAbstractPU<boolean
    // px外屏
    >;
    get isSmallSquareScreen() {
        return this.__isSmallSquareScreen.get();
    }
    set isSmallSquareScreen(newValue: boolean) {
        this.__isSmallSquareScreen.set(newValue);
    }
    // px外屏
    private __isPXSecScreen: ObservedPropertyAbstractPU<boolean
    // 判断设置是否清楚数据
    >;
    get isPXSecScreen() {
        return this.__isPXSecScreen.get();
    }
    set isPXSecScreen(newValue: boolean) {
        this.__isPXSecScreen.set(newValue);
    }
    // 判断设置是否清楚数据
    private __isClearData: ObservedPropertyAbstractPU<boolean
    // 列表是否滚动中
    >;
    get isClearData() {
        return this.__isClearData.get();
    }
    set isClearData(newValue: boolean) {
        this.__isClearData.set(newValue);
    }
    // 列表是否滚动中
    private __isRolling: ObservedPropertyAbstractPU<boolean
    // 高度断点
    >;
    get isRolling() {
        return this.__isRolling.get();
    }
    set isRolling(newValue: boolean) {
        this.__isRolling.set(newValue);
    }
    // 高度断点
    private __heightBp: ObservedPropertyAbstractPU<HeightBreakpoint>;
    get heightBp() {
        return this.__heightBp.get();
    }
    set heightBp(newValue: HeightBreakpoint) {
        this.__heightBp.set(newValue);
    }
    // 宽度断点
    private __widthBp: ObservedPropertyAbstractPU<WidthBreakpoint>;
    get widthBp() {
        return this.__widthBp.get();
    }
    set widthBp(newValue: WidthBreakpoint) {
        this.__widthBp.set(newValue);
    }
    private __paddingButtom: ObservedPropertyAbstractPU<number | Resource
    // 主界面当前index
    >;
    get paddingButtom() {
        return this.__paddingButtom.get();
    }
    set paddingButtom(newValue: number | Resource) {
        this.__paddingButtom.set(newValue);
    }
    // 主界面当前index
    private __tabCurrentPage: ObservedPropertyAbstractPU<number>;
    get tabCurrentPage() {
        return this.__tabCurrentPage.get();
    }
    set tabCurrentPage(newValue: number) {
        this.__tabCurrentPage.set(newValue);
    }
    private __curOffsetRemain: ObservedPropertyAbstractPU<number
    // px外屏隐藏top和tab偏移
    >;
    get curOffsetRemain() {
        return this.__curOffsetRemain.get();
    }
    set curOffsetRemain(newValue: number) {
        this.__curOffsetRemain.set(newValue);
    }
    // px外屏隐藏top和tab偏移
    private __heightTop: ObservedPropertyAbstractPU<number
    // page内顶部标题偏移量
    >;
    get heightTop() {
        return this.__heightTop.get();
    }
    set heightTop(newValue: number) {
        this.__heightTop.set(newValue);
    }
    // page内顶部标题偏移量
    private __curYOffset: ObservedPropertyAbstractPU<number
    /**
     * 配置页面内长按触摸项
     */
    >;
    get curYOffset() {
        return this.__curYOffset.get();
    }
    set curYOffset(newValue: number) {
        this.__curYOffset.set(newValue);
    }
    /**
     * 配置页面内长按触摸项
     */
    private __currentTouchConfigData: ObservedPropertyAbstractPU<ConfigData>;
    get currentTouchConfigData() {
        return this.__currentTouchConfigData.get();
    }
    set currentTouchConfigData(newValue: ConfigData) {
        this.__currentTouchConfigData.set(newValue);
    }
    private __configList: ObservedPropertyAbstractPU<Array<ConfigData>>;
    get configList() {
        return this.__configList.get();
    }
    set configList(newValue: Array<ConfigData>) {
        this.__configList.set(newValue);
    }
    private __currentRadioConfigData: ObservedPropertyObjectPU<ConfigData>;
    get currentRadioConfigData() {
        return this.__currentRadioConfigData.get();
    }
    set currentRadioConfigData(newValue: ConfigData) {
        this.__currentRadioConfigData.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Stack.create();
            Stack.width('100%');
            Stack.height('100%');
            Stack.alignContent(Alignment.Top);
            Stack.bindSheet({ value: this.isShowConfigAddBindSheet, changeEvent: newValue => { this.isShowConfigAddBindSheet = newValue; } }, { builder: () => {
                    this.ConfigAddBindSheetBuilder.call(this);
                } }, {
                detents: [BIND_SHEET_CONTAINER_HEIGHT, SheetSize.LARGE,],
                backgroundColor: { "id": 16777248, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                showClose: this.isShowQRCode ? false : true,
                dragBar: false,
                preferType: SheetType.CENTER,
                scrollSizeMode: ScrollSizeMode.CONTINUOUS,
                // width: ('100%'),
                /**
                 * 半模态页面显示（动画开始前）回调函数
                 *    目前`通过URL(链接)导入`和`长按配置编辑`是用的同一个页面，
                 *    这里主要是用来处理拉起`bindSheet`之后可以等`bindSheet`渲染完毕之后正确传入路由信息
                 */
                onWillAppear: () => {
                    hilog.info(0xC020, componentName, `#button#bindSheet#onWillAppear() executed`);
                    this.isEnableIndexForegroundBlur = true;
                    this.onForwardChange();
                },
                // 半模态页面交互式关闭回调函数
                onWillDismiss: async (dismissSheet: DismissSheetAction) => {
                    // 在二级路由页面内只能通过点击关闭按钮才能关掉bindSheet
                    if (this.configAddPageInfos.size() <= 0 || (dismissSheet.reason === DismissReason.CLOSE_BUTTON)) {
                        dismissSheet.dismiss();
                        this.isShowConfigAddBindSheet = false;
                        if (!this.isConfigDownloading) {
                            this.isEnableIndexForegroundBlur = false;
                        }
                        this.configAddPageInfos.clear(true);
                        hilog.info(0xC020, componentName, `#button#bindSheet#onWillDismiss()#关闭配置添加按钮的bindSheet：DismissSheetAction.dismiss()`);
                    }
                },
                onWillDisappear: async () => { },
                onDisappear: () => {
                    Xb_PreferenceUtil.save().then(() => {
                        hilog.debug(0xC020, componentName, `#button#bindSheet#onWillDismiss()#关闭配置添加按钮的bindSheet：DismissSheetAction.dismiss() ConfigSort Xb_PreferenceUtil 首选项保存成功`);
                    }).catch((err: BusinessError) => {
                        hilog.error(0xC020, componentName, `#button#bindSheet#onWillDismiss()#关闭配置添加按钮的bindSheet：DismissSheetAction.dismiss() ConfigSort Xb_PreferenceUtil 首选项保存失败，message：${err.message}`);
                    });
                    setTimeout(() => {
                        // 关闭背景模糊 TODO 此处添加是为了修复添加配置关闭半模态背景模糊未能正常关闭
                        if (!this.isConfigDownloading) {
                            this.isEnableIndexForegroundBlur = false;
                        }
                    });
                    this.isNewConfig = false;
                    this.showSort = false;
                    this.onForwardChange();
                    this.configStr = ''; // 置空配置文本
                },
                // onWillSpringBackWhenDismiss: (springBack: SpringBackAction) => {
                //   springBack.springBack()
                // },
                onWillSpringBackWhenDismiss: () => { } // 注册回调，防止反向回弹
            });
        }, Stack);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            // 标题栏 START
            if (!this.isPXSecScreen) {
                this.ifElseBranchUpdateFunction(0, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Row.create();
                        Row.zIndex(1);
                        Row.backgroundColor({ "id": 16777248, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                        Row.height(this.curYOffset < 0 ? TAB_CONTENT_TITLE_HEIGHT - this.curYOffset : TAB_CONTENT_TITLE_HEIGHT);
                        Row.position({ x: 0, y: this.isRolling && this.isSmallSquareScreen && !this.windowMode ? this.heightTop : 0 });
                        Row.alignItems(this.windowMode ? VerticalAlign.Top : VerticalAlign.Center);
                        Row.width('100%');
                    }, Row);
                    {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            if (isInitialRender) {
                                let componentCall = new TopBar(this, {
                                    title: getResourceString({ "id": 16777223, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, this),
                                    titleFontSize: this.curYOffset < 0 ? TAB_CONTENT_TITLE_FONT_SIZE - this.curYOffset / 100 : TAB_CONTENT_TITLE_FONT_SIZE,
                                    icons: [{ "id": 125831440, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, { "id": 125831551, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }],
                                    iconSize: this.windowMode ? 18 : (this.curYOffset < 0 ? 24 - this.curYOffset / 100 : 24),
                                    // 排序按钮
                                    onFClick: () => {
                                        this.showSort = true;
                                        this.isShowConfigAddBindSheet = true;
                                    },
                                    // 刷新图标
                                    onSClick: () => {
                                        this.isON.toggleAnim = true;
                                        hilog.info(0xC001, componentName, `#button#onClick()#更新配置`);
                                        this.updateConfig();
                                    }
                                }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 442, col: 11 });
                                ViewPU.create(componentCall);
                                let paramsLambda = () => {
                                    return {
                                        title: getResourceString({ "id": 16777223, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, this),
                                        titleFontSize: this.curYOffset < 0 ? TAB_CONTENT_TITLE_FONT_SIZE - this.curYOffset / 100 : TAB_CONTENT_TITLE_FONT_SIZE,
                                        icons: [{ "id": 125831440, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, { "id": 125831551, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }],
                                        iconSize: this.windowMode ? 18 : (this.curYOffset < 0 ? 24 - this.curYOffset / 100 : 24),
                                        // 排序按钮
                                        onFClick: () => {
                                            this.showSort = true;
                                            this.isShowConfigAddBindSheet = true;
                                        },
                                        // 刷新图标
                                        onSClick: () => {
                                            this.isON.toggleAnim = true;
                                            hilog.info(0xC001, componentName, `#button#onClick()#更新配置`);
                                            this.updateConfig();
                                        }
                                    };
                                };
                                componentCall.paramsGenerator_ = paramsLambda;
                            }
                            else {
                                this.updateStateVarsOfChildByElmtId(elmtId, {
                                    title: getResourceString({ "id": 16777223, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, this),
                                    titleFontSize: this.curYOffset < 0 ? TAB_CONTENT_TITLE_FONT_SIZE - this.curYOffset / 100 : TAB_CONTENT_TITLE_FONT_SIZE,
                                    icons: [{ "id": 125831440, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, { "id": 125831551, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }],
                                    iconSize: this.windowMode ? 18 : (this.curYOffset < 0 ? 24 - this.curYOffset / 100 : 24)
                                });
                            }
                        }, { name: "TopBar" });
                    }
                    Row.pop();
                });
            }
            // 标题栏 END
            // 主体 START
            else {
                this.ifElseBranchUpdateFunction(1, () => {
                });
            }
        }, If);
        If.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            // 标题栏 END
            // 主体 START
            Stack.create({ alignContent: this.stackAlignContentAlignment });
            // 标题栏 END
            // 主体 START
            Stack.backgroundColor({ "id": 16777248, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            // 标题栏 END
            // 主体 START
            Stack.width('100%');
            // 标题栏 END
            // 主体 START
            Stack.height('100%');
        }, Stack);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            if (this.configList.length === 0) {
                this.ifElseBranchUpdateFunction(0, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        __Common__.create();
                        __Common__.height('90%');
                    }, __Common__);
                    {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            if (isInitialRender) {
                                let componentCall = new Nothing(this, { name: { "id": 16777223, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 472, col: 11 });
                                ViewPU.create(componentCall);
                                let paramsLambda = () => {
                                    return {
                                        name: { "id": 16777223, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }
                                    };
                                };
                                componentCall.paramsGenerator_ = paramsLambda;
                            }
                            else {
                                this.updateStateVarsOfChildByElmtId(elmtId, {
                                    name: { "id": 16777223, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }
                                });
                            }
                        }, { name: "Nothing" });
                    }
                    __Common__.pop();
                });
            }
            else {
                this.ifElseBranchUpdateFunction(1, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Column.create();
                    }, Column);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        If.create();
                        if (!this.isPXSecScreen) {
                            this.ifElseBranchUpdateFunction(0, () => {
                                this.observeComponentCreation2((elmtId, isInitialRender) => {
                                    Blank.create();
                                    Blank.height((this.isRolling && this.isSmallSquareScreen) ? this.heightTop : TAB_CONTENT_TITLE_HEIGHT);
                                }, Blank);
                                Blank.pop();
                            });
                        }
                        else {
                            this.ifElseBranchUpdateFunction(1, () => {
                            });
                        }
                    }, If);
                    If.pop();
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Refresh.create({ refreshing: { value: this.isRefreshing, changeEvent: newValue => { this.isRefreshing = newValue; } }, builder: this.LoadingBuilder.bind(this) });
                        Refresh.zIndex(2);
                        Refresh.refreshOffset(64);
                        Refresh.pullToRefresh(true);
                        Refresh.onRefreshing(() => {
                            this.updateConfig();
                        });
                    }, Refresh);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        // 配置列表 START
                        List.create({ space: LIST_SPACE });
                        // 配置列表 START
                        List.zIndex(3);
                        // 配置列表 START
                        List.width('100%');
                        // 配置列表 START
                        List.height('100%');
                        // 配置列表 START
                        List.onScrollFrameBegin((offset: number) => {
                            this.curOffsetRemain += offset;
                            if (offset > 0) {
                                this.isRolling = true;
                            }
                            else if (offset < 0) {
                                this.isRolling = false;
                                this.curOffsetRemain = 0;
                            }
                            // console.info("offsetRemain:" + offset + '/' +this.curOffsetRemain);
                            return { offsetRemain: offset };
                        });
                        // 配置列表 START
                        List.padding({
                            left: TabContentTitleMargin(this.widthBp, this.heightBp),
                            right: TabContentTitleMargin(this.widthBp, this.heightBp),
                        });
                        // 配置列表 START
                        List.contentEndOffset(this.isPXSecScreen ? 65 : CONTENT_END_OFFSET_150);
                        // 配置列表 START
                        List.lanes(ListLanes(this.widthBp, this.heightBp), ListGutter(this.widthBp, this.heightBp));
                        // 配置列表 START
                        List.edgeEffect(EdgeEffect.Spring);
                        // 配置列表 START
                        List.chainAnimation(true);
                        // 配置列表 START
                        List.nestedScroll({
                            scrollForward: NestedScrollMode.PARENT_FIRST,
                            scrollBackward: NestedScrollMode.SELF_FIRST
                        });
                    }, List);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        ForEach.create();
                        const forEachItemGenFunction = (_item, index: number) => {
                            const item = _item;
                            {
                                const itemCreation = (elmtId, isInitialRender) => {
                                    ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
                                    ListItem.create(deepRenderFunction, true);
                                    if (!isInitialRender) {
                                        ListItem.pop();
                                    }
                                    ViewStackProcessor.StopGetAccessRecording();
                                };
                                const itemCreation2 = (elmtId, isInitialRender) => {
                                    ListItem.create(deepRenderFunction, true);
                                    ListItem.width('100%');
                                    ListItem.backgroundColor({ "id": 16777249, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                                    ListItem.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
                                    ListItem.borderRadius(BORDER_RADIUS_20);
                                    ListItem.hoverEffect(HoverEffect.Scale);
                                    ListItem.transition(this.isON.toggleAnim ? null : customAnimationUtil.isScaleTran(index, ObservedObject.GetRawObject(this.uiConfig)));
                                    ListItem.bindContextMenu(this.isShowMenu && this.currentTouchConfigData.configId == item.configId, this.isPXSecScreen ? null : { builder: () => {
                                            this.configListLongPressCtxMenuBuilder.call(this, item);
                                        } }, {
                                        preview: MenuPreviewMode.IMAGE,
                                        previewAnimationOptions: { scale: [1.0, 1.1] },
                                        placement: Placement.BottomRight,
                                        aboutToAppear: () => {
                                            if (this.uiConfig.isVibrate) {
                                                customVibrator.vibratorTriggerOfHapticClockTimer();
                                            }
                                        },
                                        aboutToDisappear: () => {
                                            this.isShowMenu = false;
                                        }
                                    });
                                    globalThis.Gesture.create(GesturePriority.Low);
                                    TapGesture.create({ count: 2 });
                                    TapGesture.onAction(async (event: GestureEvent) => {
                                        if (event) {
                                            hilog.debug(0xC020, componentName, `#TapGesture 配置列表项双击`);
                                            await this.loadConfig(item);
                                        }
                                    });
                                    TapGesture.pop();
                                    globalThis.Gesture.pop();
                                    globalThis.Gesture.create(GesturePriority.Low);
                                    TapGesture.create({ count: 1 });
                                    TapGesture.onAction((event: GestureEvent) => {
                                        if (event) {
                                            hilog.debug(0xC020, componentName, `#TapGesture 配置列表项单击`);
                                            this.isON.toggleAnim = true;
                                            if (this.currentRadioConfigData.configId != item.configId) {
                                                this.loadConfig(item);
                                            }
                                            // 更新当前选中
                                            this.currentRadioConfigData = item;
                                        }
                                    });
                                    TapGesture.pop();
                                    globalThis.Gesture.pop();
                                    ListItem.onMouse((event) => {
                                        // 右键菜单
                                        if (event.action == MouseAction.Press && event.button == MouseButton.Right) {
                                            this.currentTouchConfigData = item;
                                            this.isShowMenu = true;
                                        }
                                    });
                                    globalThis.Gesture.create(GesturePriority.Low);
                                    LongPressGesture.create();
                                    LongPressGesture.onAction(() => {
                                        // 赋值当前长按触摸列表项
                                        this.currentTouchConfigData = item;
                                        // 显示菜单
                                        this.isShowMenu = true;
                                    });
                                    LongPressGesture.pop();
                                    globalThis.Gesture.pop();
                                    ListItem.bindPopup(!!this.uiConfig.ShowConfigPop, {
                                        builder: { builder: () => {
                                                PopupBuilder.call(this, this, PopTips[1], () => {
                                                    this.uiConfig.ShowConfigPop = false;
                                                });
                                            } },
                                        onWillDismiss: false,
                                        autoCancel: false
                                    });
                                };
                                const deepRenderFunction = (elmtId, isInitialRender) => {
                                    itemCreation(elmtId, isInitialRender);
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        Stack.create();
                                        Stack.width('100%');
                                        Stack.padding(LIST_ITEM_PADDING);
                                    }, Stack);
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        // 名称
                                        Column.create({ space: 2 });
                                        // 名称
                                        Column.width('100%');
                                        // 名称
                                        Column.padding({ top: 10, bottom: 10 });
                                        // 名称
                                        Column.alignItems(HorizontalAlign.Start);
                                    }, Column);
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        Text.create(item.configName);
                                        Text.fontColor(({ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }));
                                        Text.maxLines(1);
                                        Text.textOverflow({ overflow: TextOverflow.Ellipsis });
                                    }, Text);
                                    Text.pop();
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        // Text(`当前id：${item.configId}`)
                                        // Text(`当前选中：${this.appConfig.currentProfileId}`)
                                        Row.create();
                                    }, Row);
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        Text.create({ "id": 16777245, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                                        Text.fontColor({ "id": 125830983, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                                        Text.fontSize(14);
                                    }, Text);
                                    Text.pop();
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        Text.create(convertTime(item.lastUpdateDateTime));
                                        Text.fontColor({ "id": 125830983, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                                        Text.fontSize(14);
                                        Text.maxLines(1);
                                        Text.textOverflow({ overflow: TextOverflow.Ellipsis });
                                    }, Text);
                                    Text.pop();
                                    // Text(`当前id：${item.configId}`)
                                    // Text(`当前选中：${this.appConfig.currentProfileId}`)
                                    Row.pop();
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        If.create();
                                        // 进度条组件: 使用流量 / 总流量
                                        if (item.subscriptionInfo) {
                                            this.ifElseBranchUpdateFunction(0, () => {
                                                {
                                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                                        if (isInitialRender) {
                                                            let componentCall = new CellularData(this, { info: item.subscriptionInfo }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 500, col: 27 });
                                                            ViewPU.create(componentCall);
                                                            let paramsLambda = () => {
                                                                return {
                                                                    info: item.subscriptionInfo
                                                                };
                                                            };
                                                            componentCall.paramsGenerator_ = paramsLambda;
                                                        }
                                                        else {
                                                            this.updateStateVarsOfChildByElmtId(elmtId, {
                                                                info: item.subscriptionInfo
                                                            });
                                                        }
                                                    }, { name: "CellularData" });
                                                }
                                            });
                                        }
                                        else {
                                            this.ifElseBranchUpdateFunction(1, () => {
                                            });
                                        }
                                    }, If);
                                    If.pop();
                                    // 名称
                                    Column.pop();
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        // 进度条 & 单选框
                                        Row.create();
                                        // 进度条 & 单选框
                                        Row.width('100%');
                                        // 进度条 & 单选框
                                        Row.justifyContent(FlexAlign.End);
                                    }, Row);
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        If.create();
                                        if (item.configId === this.appConfig.currentProfileId && this.isLoading) {
                                            this.ifElseBranchUpdateFunction(0, () => {
                                                this.observeComponentCreation2((elmtId, isInitialRender) => {
                                                    LoadingProgress.create();
                                                    LoadingProgress.color(ObservedObject.GetRawObject(this.icon_emphasize));
                                                    LoadingProgress.width(25);
                                                    LoadingProgress.height(25);
                                                    LoadingProgress.transition(customAnimationUtil.isOpacity(ObservedObject.GetRawObject(this.uiConfig)));
                                                }, LoadingProgress);
                                            });
                                        }
                                        else {
                                            this.ifElseBranchUpdateFunction(1, () => {
                                            });
                                        }
                                    }, If);
                                    If.pop();
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        Radio.create({ value: 'radio' + index, group: 'configGroup' });
                                        Radio.checked(item.configId === this.appConfig.currentProfileId);
                                        Radio.radioStyle({
                                            checkedBackgroundColor: this.icon_emphasize,
                                        });
                                        Radio.hitTestBehavior(HitTestMode.None);
                                    }, Radio);
                                    // 进度条 & 单选框
                                    Row.pop();
                                    Stack.pop();
                                    ListItem.pop();
                                };
                                this.observeComponentCreation2(itemCreation2, ListItem);
                                ListItem.pop();
                            }
                        };
                        this.forEachUpdateFunction(elmtId, this.configList, forEachItemGenFunction, undefined, true, false);
                    }, ForEach);
                    ForEach.pop();
                    // 配置列表 START
                    List.pop();
                    Refresh.pop();
                    Column.pop();
                });
            }
        }, If);
        If.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            // 悬浮新增配置按钮 START — 按位置选择转场方向，三元简化判断
            if (this.uiConfig.isFloatButtonVisible) {
                this.ifElseBranchUpdateFunction(0, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        If.create();
                        if (!this.isPXSecScreen && this.uiConfig.tabStyle == 'custom') {
                            this.ifElseBranchUpdateFunction(0, () => {
                                this.observeComponentCreation2((elmtId, isInitialRender) => {
                                    __Common__.create();
                                    __Common__.transition(customAnimationUtil.getFloatButtonTransition(ObservedObject.GetRawObject(this.uiConfig), this.uiConfig.buttonPositionIndex));
                                    __Common__.onClick(() => { EventHub.sendEvent(EventKey.AddConfig); });
                                }, __Common__);
                                {
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        if (isInitialRender) {
                                            let componentCall = new RoundButton(this, { Icon2: { "id": 125831481, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 634, col: 13 });
                                            ViewPU.create(componentCall);
                                            let paramsLambda = () => {
                                                return {
                                                    Icon2: { "id": 125831481, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }
                                                };
                                            };
                                            componentCall.paramsGenerator_ = paramsLambda;
                                        }
                                        else {
                                            this.updateStateVarsOfChildByElmtId(elmtId, {
                                                Icon2: { "id": 125831481, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }
                                            });
                                        }
                                    }, { name: "RoundButton" });
                                }
                                __Common__.pop();
                            });
                        }
                        else {
                            this.ifElseBranchUpdateFunction(1, () => {
                                this.FloatAddButton.bind(this)();
                            });
                        }
                    }, If);
                    If.pop();
                });
            }
            // 悬浮新增配置按钮 END
            else {
                this.ifElseBranchUpdateFunction(1, () => {
                });
            }
        }, If);
        If.pop();
        // 标题栏 END
        // 主体 START
        Stack.pop();
        Stack.pop();
    }
    // ContextMenu 列表项 动态替换用
    private configListLongPressCtxMenuForUnFavorite: LongPressContextMenuItem;
    private configListLongPressCtxMenuForFavorite: LongPressContextMenuItem;
    /* 自定义弹框 */
    // 配置是否保存弹框 Controller定义
    private configCustomDialogController: CustomDialogController | null;
    /**
     * ContextMenu列表项集合
     */
    configListLongPressCtxMenus(item: ConfigData): Array<LongPressContextMenuItem> {
        return [
            { symbolStartIcon: new SymbolGlyphModifier({ "id": 125831624, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }).fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE).fontColor([{ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }]),
                content: { "id": 16777224, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                callback: () => {
                    this.isShowQRCode = false;
                    this.isNewConfig = false;
                    this.editConfig();
                } },
            ...(!item.disableUpdate ? [
                { symbolStartIcon: new SymbolGlyphModifier({ "id": 125831551, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" })
                        .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE).fontColor([{ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }]),
                    content: { "id": 16777244, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, callback: () => { this.updateConfig(this.currentTouchConfigData.configId); } } as LongPressContextMenuItem
            ] : []),
            ...(this.currentTouchConfigData.sourceType === ProfileType.Url ? [
                { symbolStartIcon: new SymbolGlyphModifier({ "id": 125831499, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" })
                        .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE).fontColor([{ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }]),
                    content: { "id": 16777240, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, callback: () => { this.shareConfigUrl(); } } as LongPressContextMenuItem
            ] : []),
            // 二维码分享
            { symbolStartIcon: new SymbolGlyphModifier({ "id": 125831600, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" })
                    .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE).fontColor([{ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }]),
                content: { "id": 16777237, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, callback: () => { this.generateQRcode(); } },
            // 导出配置文件
            { symbolStartIcon: new SymbolGlyphModifier({ "id": 125831910, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" })
                    .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE).fontColor([{ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }]),
                content: { "id": 16777233, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, callback: () => { this.exportConfig(); } },
            /*  { symbolStartIcon: new SymbolGlyphModifier($r('sys.symbol.plus_square_on_square'))
                .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE).fontColor([$r('sys.color.font_primary')]),
                content: $r('app.string.create_copy'), callback: ()=>{ this.generateQRcode() }
              },*/
        ];
    }
    FloatAddButton(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
            Row.backdropBlur(this.uiConfig.isBlurr ? 30 : 0);
            Row.backgroundColor(this.uiConfig.isBlurr ? { "id": 16777250, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } : { "id": 16777248, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            Row.borderRadius(30);
            Row.margin({ bottom: 20 });
            Row.size({ width: 50, height: 50 });
            Row.justifyContent(FlexAlign.Center);
            Row.alignItems(VerticalAlign.Center);
            Row.shadow({
                radius: 15,
                offsetY: 4,
                color: { "id": 16777252, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                type: ShadowType.COLOR
            });
            Row.margin({
                bottom: Xb_TabBarVertical(this.widthBp, this.heightBp) || this.uiConfig.tabStyle == 'custom' ? 15 : this.bottomRectHeight == 0 ? 85 : this.bottomRectHeight + 63.5,
                right: 15,
                left: 15,
            });
            Row.onClick(() => {
                // 新配置
                EventHub.sendEvent(EventKey.AddConfig);
            });
            Row.transition(customAnimationUtil.getFloatButtonTransition(ObservedObject.GetRawObject(this.uiConfig), this.uiConfig.buttonPositionIndex));
        }, Row);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            SymbolGlyph.create({ "id": 125831481, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            SymbolGlyph.fontSize(27);
            SymbolGlyph.fontColor([icon_tertiary]);
        }, SymbolGlyph);
        Row.pop();
    }
    /**
     * 配置页面ListItem的长按ContextMenu的Builder
     */
    configListLongPressCtxMenuBuilder(data: ConfigData, parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            // 当菜单过多（横屏时），自身的滚动条不支持回弹，套一层Scroll
            Scroll.create();
            // 当菜单过多（横屏时），自身的滚动条不支持回弹，套一层Scroll
            Scroll.nestedScroll({ scrollForward: NestedScrollMode.SELF_FIRST, scrollBackward: NestedScrollMode.SELF_FIRST });
            // 当菜单过多（横屏时），自身的滚动条不支持回弹，套一层Scroll
            Scroll.edgeEffect(EdgeEffect.Spring);
        }, Scroll);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Menu.create();
            Menu.backgroundColor({ "id": 16777249, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            Menu.borderRadius(BORDER_RADIUS_20);
            Menu.menuItemDivider({ strokeWidth: LengthMetrics.vp(.4), color: { "id": 125831013, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
            Menu.width(200);
            Menu.height('auto');
            Menu.transition(this.uiConfig.isAnimation ? TransitionEffect.asymmetric(TransitionEffect.move(TransitionEdge.END), TransitionEffect.move(TransitionEdge.START))
                .animation({ duration: this.uiConfig.animationSpeed, curve: Curve.Ease })
                .combine(TransitionEffect.OPACITY) : null);
        }, Menu);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            // 根据当前配置信息来判断显示[收藏 /取消收藏] 单独处理 START
            if (this.favoriteProfiles.has(data.configId)) {
                this.ifElseBranchUpdateFunction(0, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        MenuItem.create({
                            symbolStartIcon: this.configListLongPressCtxMenuForUnFavorite.symbolStartIcon!
                                .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE),
                            content: this.configListLongPressCtxMenuForUnFavorite.content
                        });
                        MenuItem.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
                        MenuItem.contentFontColor({ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                        MenuItem.onClick(() => {
                            this.configListLongPressCtxMenuForUnFavorite.callback();
                        });
                    }, MenuItem);
                    MenuItem.pop();
                });
            }
            else {
                this.ifElseBranchUpdateFunction(1, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        MenuItem.create({
                            symbolStartIcon: this.configListLongPressCtxMenuForFavorite.symbolStartIcon!
                                .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE),
                            content: this.configListLongPressCtxMenuForFavorite.content
                        });
                        MenuItem.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
                        MenuItem.contentFontColor({ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                        MenuItem.onClick(() => {
                            this.configListLongPressCtxMenuForFavorite.callback();
                        });
                    }, MenuItem);
                    MenuItem.pop();
                });
            }
        }, If);
        If.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            // 根据当前配置信息来判断显示[收藏 /取消收藏] 单独处理 END
            // 遍历其他菜单 START
            ForEach.create();
            const forEachItemGenFunction = _item => {
                const item = _item;
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    MenuItem.create({ symbolStartIcon: item?.symbolStartIcon, content: item?.content });
                    MenuItem.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
                    MenuItem.contentFontColor({ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                    MenuItem.onClick(() => {
                        // hilog.info(0xC002, this.componentName, `#configListLongPressCtxMenuBuilder#onClick():`)
                        item.callback();
                    });
                }, MenuItem);
                MenuItem.pop();
            };
            this.forEachUpdateFunction(elmtId, this.configListLongPressCtxMenus(data), forEachItemGenFunction);
        }, ForEach);
        // 根据当前配置信息来判断显示[收藏 /取消收藏] 单独处理 END
        // 遍历其他菜单 START
        ForEach.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            // 遍历其他菜单 END
            // 删除配置 文字红色 单独处理 START
            MenuItem.create({
                symbolStartIcon: new SymbolGlyphModifier({ "id": 125831543, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" })
                    .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE)
                    .fontColor(['#E6E54E36']),
                content: { "id": 16777231, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }
            });
            // 遍历其他菜单 END
            // 删除配置 文字红色 单独处理 START
            MenuItem.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
            // 遍历其他菜单 END
            // 删除配置 文字红色 单独处理 START
            MenuItem.contentFontColor('#E6E54E36');
            // 遍历其他菜单 END
            // 删除配置 文字红色 单独处理 START
            MenuItem.onClick(() => {
                this.configCustomDialogController?.open();
                this.isEnableIndexForegroundBlur = true;
            });
        }, MenuItem);
        // 遍历其他菜单 END
        // 删除配置 文字红色 单独处理 START
        MenuItem.pop();
        Menu.pop();
        // 当菜单过多（横屏时），自身的滚动条不支持回弹，套一层Scroll
        Scroll.pop();
    }
    ConfigAddBindSheetBuilder(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            if (this.isNewConfig) {
                this.ifElseBranchUpdateFunction(0, () => {
                    {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            if (isInitialRender) {
                                let componentCall = new ConfigAdd(this, {}, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 902, col: 7 });
                                ViewPU.create(componentCall);
                                let paramsLambda = () => {
                                    return {};
                                };
                                componentCall.paramsGenerator_ = paramsLambda;
                            }
                            else {
                                this.updateStateVarsOfChildByElmtId(elmtId, {});
                            }
                        }, { name: "ConfigAdd" });
                    }
                });
            }
            // 分享二维码
            else if (this.isShowQRCode) {
                this.ifElseBranchUpdateFunction(1, () => {
                    {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            if (isInitialRender) {
                                let componentCall = new QRCodeDialog(this, {
                                    theDialogTitle: { "id": 16777239, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                                    QRCodeParam: {
                                        // TODO PC端需适配
                                        url: this.currentTouchConfigData.configUrl,
                                        name: this.currentTouchConfigData.configName,
                                    },
                                    confirm: async () => {
                                        // 关闭二级界面
                                        this.isShowQRCode = false;
                                        this.isShowConfigAddBindSheet = false;
                                    },
                                    cancel: () => {
                                        // 关闭二级界面
                                        this.isShowQRCode = false;
                                        this.isShowConfigAddBindSheet = false;
                                    },
                                }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 906, col: 7 });
                                ViewPU.create(componentCall);
                                let paramsLambda = () => {
                                    return {
                                        theDialogTitle: { "id": 16777239, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                                        QRCodeParam: {
                                            // TODO PC端需适配
                                            url: this.currentTouchConfigData.configUrl,
                                            name: this.currentTouchConfigData.configName,
                                        },
                                        confirm: async () => {
                                            // 关闭二级界面
                                            this.isShowQRCode = false;
                                            this.isShowConfigAddBindSheet = false;
                                        },
                                        cancel: () => {
                                            // 关闭二级界面
                                            this.isShowQRCode = false;
                                            this.isShowConfigAddBindSheet = false;
                                        }
                                    };
                                };
                                componentCall.paramsGenerator_ = paramsLambda;
                            }
                            else {
                                this.updateStateVarsOfChildByElmtId(elmtId, {
                                    theDialogTitle: { "id": 16777239, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
                                    QRCodeParam: {
                                        // TODO PC端需适配
                                        url: this.currentTouchConfigData.configUrl,
                                        name: this.currentTouchConfigData.configName,
                                    }
                                });
                            }
                        }, { name: "QRCodeDialog" });
                    }
                });
            }
            else if (this.showSort) {
                this.ifElseBranchUpdateFunction(2, () => {
                    // 排序窗口
                    this.SortBuilder.bind(this)();
                });
            }
            else {
                this.ifElseBranchUpdateFunction(3, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        If.create();
                        // 二级编辑界面
                        if (this.isFromFile) {
                            this.ifElseBranchUpdateFunction(0, () => {
                                {
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        if (isInitialRender) {
                                            let componentCall = new 
                                            // 文件导入的
                                            ImportConfigFromFile(this, { configData: this.currentTouchConfigData }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 931, col: 9 });
                                            ViewPU.create(componentCall);
                                            let paramsLambda = () => {
                                                return {
                                                    configData: this.currentTouchConfigData
                                                };
                                            };
                                            componentCall.paramsGenerator_ = paramsLambda;
                                        }
                                        else {
                                            this.updateStateVarsOfChildByElmtId(elmtId, {
                                                configData: this.currentTouchConfigData
                                            });
                                        }
                                    }, { name: "ImportConfigFromFile" });
                                }
                            });
                        }
                        else {
                            this.ifElseBranchUpdateFunction(1, () => {
                                {
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        if (isInitialRender) {
                                            let componentCall = new 
                                            // URL导入的
                                            ImportConfigFromURL(this, { configData: this.currentTouchConfigData }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 934, col: 9 });
                                            ViewPU.create(componentCall);
                                            let paramsLambda = () => {
                                                return {
                                                    configData: this.currentTouchConfigData
                                                };
                                            };
                                            componentCall.paramsGenerator_ = paramsLambda;
                                        }
                                        else {
                                            this.updateStateVarsOfChildByElmtId(elmtId, {
                                                configData: this.currentTouchConfigData
                                            });
                                        }
                                    }, { name: "ImportConfigFromURL" });
                                }
                            });
                        }
                    }, If);
                    If.pop();
                });
            }
        }, If);
        If.pop();
    }
    LoadingBuilder(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            LoadingProgress.create();
            LoadingProgress.color(ObservedObject.GetRawObject(this.icon_emphasize));
            LoadingProgress.width(40);
            LoadingProgress.height(40);
            LoadingProgress.margin({ bottom: 15 });
        }, LoadingProgress);
    }
    SortBuilder(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
            Column.width('100%');
            Column.height('100%');
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
            Row.width('100%');
            Row.margin({ left: 5 });
            Row.alignItems(VerticalAlign.Center);
            Row.height(BIND_SHEET_TITLE_CONTAINER_HEIGHT + BIND_SHEET_DRAG_BAR_HEIGHT);
            Row.padding({
                left: NavigationListPadding(this.widthBp, this.heightBp),
                right: NavigationListPadding(this.widthBp, this.heightBp)
            });
        }, Row);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create({ "id": 16777241, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            globalThis.Context.animation({ duration: this.uiConfig.animationSpeed, curve: Curve.Ease });
            Text.fontWeight(FontWeight.Bold);
            Text.fontColor({ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            Text.textOverflow({ overflow: TextOverflow.MARQUEE });
            globalThis.Context.animation(null);
        }, Text);
        Text.pop();
        Row.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            List.create({ space: LIST_SPACE });
            List.width('100%');
            List.contentEndOffset(86);
            List.padding({
                left: NavigationListPadding(this.widthBp, this.heightBp),
                right: NavigationListPadding(this.widthBp, this.heightBp)
            });
        }, List);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            ForEach.create();
            ForEach.onMove((from: number, to: number) => {
                let tmp = this.configList.splice(from, 1);
                this.configList.splice(to, 0, tmp[0]);
                // 更新保存的排序表
                Xb_PreferenceUtil.putSync('config_sort', JSON.stringify(this.configList.map(id => id.configId)));
                console.info(`ConfigSort  Xb_PreferenceUtil config_sort 更新为，${JSON.stringify(this.configList.map(id => id.configId))}`);
            });
            const forEachItemGenFunction = (_item, index: number) => {
                const item = _item;
                {
                    const itemCreation = (elmtId, isInitialRender) => {
                        ViewStackProcessor.StartGetAccessRecordingFor(elmtId);
                        ListItem.create(deepRenderFunction, true);
                        if (!isInitialRender) {
                            ListItem.pop();
                        }
                        ViewStackProcessor.StopGetAccessRecording();
                    };
                    const itemCreation2 = (elmtId, isInitialRender) => {
                        ListItem.create(deepRenderFunction, true);
                        ListItem.width('100%');
                        ListItem.backgroundColor({ "id": 16777249, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                        ListItem.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
                        ListItem.borderRadius(BORDER_RADIUS_20);
                        ListItem.hoverEffect(HoverEffect.Scale);
                        ListItem.transition(this.isON.toggleAnim ? null : customAnimationUtil.isScaleTran(index, ObservedObject.GetRawObject(this.uiConfig)));
                    };
                    const deepRenderFunction = (elmtId, isInitialRender) => {
                        itemCreation(elmtId, isInitialRender);
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Row.create();
                            Row.height(56);
                            Row.width('100%');
                            Row.padding(LIST_ITEM_PADDING);
                            Row.justifyContent(FlexAlign.SpaceBetween);
                        }, Row);
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            // 名称
                            Text.create(item.configName);
                            // 名称
                            Text.fontColor(({ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }));
                            // 名称
                            Text.maxLines(1);
                            // 名称
                            Text.textOverflow({ overflow: TextOverflow.Ellipsis });
                        }, Text);
                        // 名称
                        Text.pop();
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            SymbolGlyph.create({ "id": 125831440, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                            SymbolGlyph.fontSize(24);
                            SymbolGlyph.fontColor([{ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }]);
                        }, SymbolGlyph);
                        Row.pop();
                        ListItem.pop();
                    };
                    this.observeComponentCreation2(itemCreation2, ListItem);
                    ListItem.pop();
                }
            };
            this.forEachUpdateFunction(elmtId, this.configList, forEachItemGenFunction, (item: ConfigData) => item.configId, true, false);
        }, ForEach);
        ForEach.pop();
        List.pop();
        Column.pop();
    }
    async aboutToAppear(): Promise<void> {
        hilog.info(0xC000, componentName, `#aboutToAppear executed`);
        // 初始化当前选中项数据
        this.currentRadioConfigData = this.configList.find((data: ConfigData) => data.configId === this.appConfig.currentProfileId) ?? NewConfigData;
        // 初始化分享工具
        Xb_Share.inputInfo(this.getUIContext());
        // 注册打开添加配置弹窗事件
        EventHub.on(EventKey.AddConfig, async () => {
            this.isNewConfig = true;
            this.currentTouchConfigData = NewConfigData;
            this.isShowConfigAddBindSheet = true;
            this.isEnableIndexForegroundBlur = true;
            if (this.uiConfig.isVibrate) {
                customVibrator.vibratorTriggerOfHapticClockTimer();
            }
        });
    }
    async onDidBuild(): Promise<void> {
        await this.openShare();
    }
    async onIndexChange() {
        if (this.configList.length !== 0 && this.tabCurrentPage === 2) {
            await ClashViewModel.updateProfileConfig(null);
            Xb_ToastUtil.showToast({ message: '更新所有配置成功' });
        }
    }
    async loadConfig(item: ConfigData, needUpdated: boolean = true) {
        hilog.info(0xC011, componentName, `EditConfig UpdateConfig #loadConfig#加载配置: configName is [${this.currentTouchConfigData.configName}] 是否需要更新：${needUpdated && !item.disableUpdate} 更新时间：${item.updateTime}`);
        if (this.isLoading)
            return;
        this.getUIContext().animateTo({
            duration: this.uiConfig.animationSpeed,
            iterations: 1,
            playMode: PlayMode.Normal,
            curve: Curve.Ease,
        }, () => {
            this.isLoading = true;
        });
        this.appConfig.currentProfileName = item.configName;
        this.appConfig.currentProfileId = item.configId;
        try {
            if (needUpdated && !item.disableUpdate) {
                ClashViewModel.updateProfileConfig(item.configId);
            }
            await ClashViewModel.socketProxy.reset();
            await ClashViewModel.loadConfig(false, '配置页-加载');
            hilog.info(0xC011, componentName, `EditConfig #loadConfig#更新并加载完成 configName is [${this.currentTouchConfigData.configName}]`);
        }
        catch (e) {
            console.debug("loadConfigList", e.message, JSON.stringify(e));
            Xb_ToastUtil.showToast({ message: `${getResourceString({ "id": 16777235, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, this)}` + (e.message || e), duration: 3000 });
        }
        this.getUIContext().animateTo({
            duration: this.uiConfig.animationSpeed,
            iterations: 1,
            playMode: PlayMode.Normal,
            curve: Curve.Ease,
        }, () => {
            this.isLoading = false;
        });
    }
    async onConfigListChange() {
        hilog.info(0xC011, componentName, `EditConfig #onConfigListChange#配置列表长度: [${this.configList.length}]`);
        if (!this.uiConfig.isFirstImportConfig || this.isClearData) {
            hilog.info(0xC011, componentName, `EditConfig #onConfigListChange#设置数据清除: [${this.isClearData}]`);
            this.isClearData = false;
            hilog.info(0xC011, componentName, `EditConfig #onConfigListChange#设置数据清除: [${this.isClearData}]`);
            return;
        }
        // 首次导入配置自动选中
        if (this.configList.length === 1 && this.uiConfig.isFirstImportConfig) {
            this.isON.toggleAnim = true;
            await this.loadConfig(this.configList[0]);
            this.uiConfig.isFirstImportConfig = false;
            hilog.info(0xC002, componentName, `EditConfig #onConfigListChange: 首次导入：${this.uiConfig.isFirstImportConfig} configName is [${this.configList[0].configName}]`);
        }
    }
    aboutToDisappear(): void {
        hilog.info(0xC000, componentName, `#aboutToDisappear executed`);
        EventHub.off(EventKey.AddConfig);
        this.configCustomDialogController = null;
        this.isEnableIndexForegroundBlur = false;
        hilog.info(0xC000, componentName, `#aboutToDisappear#置空[配置页\`bindSheet\`内提示是否删除配置信息的自定义弹框]`);
        Xb_Share.immersiveDisablingListening('all');
    }
    /**
     * 收藏配置
     */
    private favoriteConfig(): void {
        hilog.info(0xC010, componentName, `#favoriteConfig#收藏配置: configName is [${this.currentTouchConfigData.configName}]`);
        if (this.favoriteProfiles.size <= 5) {
            this.currentTouchConfigData.isFavorite = true;
            this.favoriteProfiles.set(this.currentTouchConfigData.configId, this.currentTouchConfigData.configName);
            Xb_ToastUtil.showToast({ message: { "id": 16777222, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
        }
        else {
            Xb_ToastUtil.showToast({ message: { "id": 16777236, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
        }
    }
    /**
     * 取消收藏配置
     */
    private cancelFavoriteConfig(): void {
        hilog.info(0xC010, componentName, `#favoriteConfig#取消收藏配置: configName is [${this.currentTouchConfigData.configName}]`);
        this.currentTouchConfigData.isFavorite = false;
        this.favoriteProfiles.delete(this.currentTouchConfigData.configId);
        Xb_ToastUtil.showToast({ message: { "id": 16777243, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
    }
    /**
     * 编辑配置
     */
    private editConfig(): void {
        hilog.info(0xC011, componentName, `#editConfig#编辑配置: configName is [${this.currentTouchConfigData.configName}]`);
        // 识别配置导入途径
        this.isFromFile = this.currentTouchConfigData.configUrl.toLowerCase().startsWith('file://');
        // 切换为二级界面
        this.isNewConfig = false;
        // 赋值为false
        this.currentTouchConfigData.isNewConfig = false;
        // 提前加载配置文本
        EventHub.sendEvent(EventKey.LoadConfigStr);
        // 显示半模态编辑弹窗
        this.isShowConfigAddBindSheet = true;
    }
    /**更新配置*/
    private async updateConfig(configId?: string): Promise<void> {
        Xb_ToastUtil.showToast({ message: { "id": 16777247, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
        this.loadConfig(this.currentRadioConfigData, !this.currentRadioConfigData.disableUpdate);
        if (configId) {
            this.isRefreshing = false;
            // 使开关点击时不触发转场动画
            this.isON.toggleAnim = true;
            await ClashViewModel.updateProfileConfig(configId);
            hilog.info(0xC012, componentName, `#updateConfig#更新配置{单个}: configUrl is [${this.currentTouchConfigData.configUrl}], configId is [${this.currentTouchConfigData.configId}]`);
        }
        else {
            // 使开关点击时不触发转场动画
            this.isON.toggleAnim = true;
            hilog.info(0xC012, componentName, `#updateConfig#更新配置{所有}`);
            await ClashViewModel.updateProfileConfig(null);
            this.isRefreshing = false;
            Xb_ToastUtil.showToast({ message: { "id": 16777246, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
        }
    }
    /**分享配置的URL*/
    private async shareConfigUrl(): Promise<void> {
        // 处理待分享数据 并调用系统分享接口 只有URL导入的配置才可以分享链接
        let shareData = await this.getShareData('touch');
        if (shareData) {
            const context: common.UIAbilityContext = this.getUIContext().getHostContext() as common.UIAbilityContext;
            Xb_Share.shareData = shareData;
            // 进行分享面板分享
            try {
                await Xb_Share.sharePlant(context);
            }
            finally {
                // TODO 现存问题，分享后的碰一碰，隔空投递的配置项会被覆盖。下面逻辑是临时解决办法
                shareData = await this.getShareData('radio');
                Xb_Share.shareData = shareData;
            }
        }
    }
    // TODO 待最低版本兼容为6.0后需绑定窗口ID
    async onForwardChange() {
        if (this.isForward && this.tabCurrentPage === 2) {
            // 前台时注册碰一碰分享事件
            Xb_Share.immersiveListening('all');
            const capabilityRegistry: harmonyShare.SendCapabilityRegistry = {
                windowId: this.mainWindowId
            };
        }
        else {
            // 后台时解除
            const capabilityRegistry: harmonyShare.SendCapabilityRegistry = {
                windowId: this.mainWindowId
            };
            // 解除隔空投递、碰一碰 分享监听事件
            Xb_Share.immersiveDisablingListening('all');
            console.log(`onForwardChange 解除碰一碰事件`);
        }
    }
    /**
     *  @description 获取分享的数据
     *  @param type touch | radio -> touch 长按选中的配置项，radio 当前选中的配置项
     */
    private async getShareData(type: 'touch' | 'radio' = 'radio'): Promise<ShareData> {
        const currentConfigData = type === 'touch' ? this.currentTouchConfigData : this.currentRadioConfigData;
        // 将背景图写入沙箱中并获取
        const ctx: Context = this.getUIContext().getHostContext() as Context;
        const filePath = ctx.filesDir + '/shared.jpg';
        // 获取应用图标
        try {
            let file = fs.openSync(filePath, fs.OpenMode.READ_WRITE | fs.OpenMode.CREATE);
            fs.writeSync(file.fd, (ctx.resourceManager.getMediaContentSync({ "id": 16777255, "type": 20000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }.id) as Uint8Array).buffer);
            fs.closeSync(file);
            hilog.info(0xC014, componentName, `将背景图写入沙箱中并获取: ${filePath}`);
        }
        catch (e) {
            hilog.error(0xC014, componentName, `error: ${JSON.stringify(e)}`);
        }
        // TODO 构造分享数据 AppLink 携带订阅 URL 完成 => 应用直达订阅分享
        const shareData: ShareData = {
            utd: utd.UniformDataType.HYPERLINK,
            content: `${APP_LINKING}?configUrl=${currentConfigData.configUrl}&configName=${currentConfigData.configName}`,
            thumbnailUri: fileUri.getUriFromPath(filePath),
            title: { "id": 16777229, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" },
            description: currentConfigData.configName,
            label: '',
            uri: ''
        };
        hilog.info(0xC014, componentName, `碰一碰 - 分享的 configUrl: ${JSON.stringify(currentConfigData)}`);
        return shareData;
    }
    /**
     * @description 开启 隔空投递、碰一碰 分享监听
     */
    private async openShare() {
        if (canIUse('SystemCapability.Collaboration.HarmonyShare')) {
            const shareData = await this.getShareData('radio');
            if (shareData) {
                Xb_Share.shareData = shareData;
                // 隔空投递、碰一碰 分享监听
                Xb_Share.immersiveListening('all');
                // Xb_ToastUtil.showToast({
                //   message: '碰一碰，分享当前配置！📄',
                //   duration: 3000
                // })
            }
        }
        else {
            Xb_ToastUtil.showToast({
                message: '当前设备暂不支持系统分享功能！'
            });
        }
    }
    /**
     * 生成配置的URL转成的二维码
     *    1. URL & 二维码 导入的配置用对应URL生成二维码
     *    2. 其他方式导入的配置不能生成二维码
     */
    private generateQRcode(): void {
        this.isShowQRCode = true;
        this.isShowConfigAddBindSheet = true;
        hilog.info(0xC014, componentName, `#generateQRcode#生成二维码: configName is [${this.currentTouchConfigData.configName}]`);
    }
    /**
     * 导出配置文件
     */
    private async exportConfig(): Promise<void> {
        hilog.info(0xC015, componentName, `#exportConfig #导出配置文件: configName is [${this.currentTouchConfigData.configName}]`);
        const context = this.getUIContext().getHostContext()!;
        // 提取配置名称
        const fileNameDotIndex = this.currentTouchConfigData.configName.lastIndexOf('.');
        const fileName = (fileNameDotIndex == -1 || fileNameDotIndex == 0) ? this.currentTouchConfigData.configName : this.currentTouchConfigData.configName.slice(0, fileNameDotIndex);
        console.debug(`待导出配置文件的 fileName: ${fileName}`);
        const sandPath = context.filesDir + `/ClashBox/profiles/` + this.currentTouchConfigData.configId + '/config.yaml';
        const sandUri = fileUri.getUriFromPath(sandPath);
        const filePicker = new picker.DocumentViewPicker(context);
        const options = new picker.DocumentSaveOptions();
        options.newFileNames = [fileName];
        options.fileSuffixChoices = ["'文档|.yaml'"];
        options.pickerMode = picker.DocumentPickerMode.DOWNLOAD;
        const result = await filePicker.save(options);
        const exportUri = fileUri.getUriFromPath(result[0] + `/${fileName}.yaml`);
        console.debug(`待导出配置文件的 沙箱目录Uri: ${sandUri} 下载目录Uri: ${exportUri}`);
        try {
            // 导出至下载文件夹
            fs.copy(sandUri, exportUri).then(() => {
                console.info("Succeeded in copying.");
                Xb_ToastUtil.showToast({ message: Xb_GetResourceString({ "id": 16777234, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, this) + `: ${result[0] + `/${fileName}.yaml`}` });
            }).catch((err: BusinessError) => {
                console.error(`Failed to copy. Code: ${err.code}, message: ${err.message}`);
                Xb_ToastUtil.showToast({ message: `配置文件导出失败. Code: ${err.code}, message: ${err.message}` });
            });
        }
        catch (error) {
            console.error(`配置文件导出失败，name：${error.name} message：${error.message}`);
            Xb_ToastUtil.showToast({ message: `配置文件导出失败. Code: ${error.code}, message: ${error.message}` });
        }
    }
    /**删除配置*/
    private async deleteConfig(): Promise<void> {
        hilog.info(0xC016, componentName, `#deleteConfig#删除配置: configName is [${this.currentTouchConfigData.configName}]`);
        await ClashViewModel.deleteProfile(this.currentTouchConfigData.configId);
        this.appConfig.currentProxyName = undefined;
        this.appConfig.currentProfileName = undefined;
        this.favoriteProfiles.delete(this.currentTouchConfigData.configId);
        // 加载配置
        EventHub.sendEvent(EventKey.FetchProfile);
        Xb_ToastUtil.showToast({ message: `${getResourceString({ "id": 16777228, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, this)}💔` });
    }
    /**
     * @description 监听选中的配置项变化
     */
    async onChangeConfig() {
        // 选中项变更重新监听分享
        try {
            Xb_Share.immersiveDisablingListening('all');
        }
        catch (e) {
            hilog.error(0xC014, componentName, `选中项变更取消监听分享 - error: ${JSON.stringify(e)}`);
        }
        await this.openShare();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "ConfigurationPage";
    }
}
export default ConfigurationPage;
registerNamedRoute(() => new ConfigurationPage(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
