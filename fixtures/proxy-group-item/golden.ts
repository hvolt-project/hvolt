if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ProxyGroupItemPage_Params {
}
interface ProxyCustomDialog_Params {
    isEnableIndexForegroundBlur?: boolean;
    icon_emphasize?: ResourceColor;
    controller?: CustomDialogController;
    cancel?: () => void;
    confirm?: () => void;
}
interface ProxyGroupItem_Params {
    icon_emphasize?: ResourceColor;
    currentTouchIndex?: number;
    tabCurrentPage?: number;
    ProxyGroupItemSelected?: ProxyCardSize;
    isShowNetworkSpeed?: boolean;
    isShowMenu?: boolean;
    //网络测速半模拟态web控制器
    controller?: webview.WebviewController;
    isEnableIndexForegroundBlur?: boolean;
    needChangeSort?: boolean;
    proxyitems?: ProxyItem[];
    proxyGroup?: number;
    proxyGroups?: ProxyGroupItemDataSource;
    proxyLazyItems?: ProxyItemDataSource;
    currentProfile?: Profile | null;
    selectedProxy?: string;
    disabled?: boolean;
    widthBp?: WidthBreakpoint;
    heightBp?: HeightBreakpoint;
    favoriteProxys?: Map<string, string>;
    appConfig?: AppConfig;
    uiConfig?: UIConfig;
    showStart?: boolean;
    isShowDelayPop?: boolean;
    isRolling?: boolean;
    curOffsetRemain?: number;
    isPXSecScreen?: boolean;
    /** 速度与延迟区别弹窗 */
    ProxyDialogController?: CustomDialogController | null;
    OnProxyChange?: ((item: ProxyItem) => void);
    proxyListLongPressCtxMenuForFavorite?: LongPressContextMenuItem;
    proxyListLongPressCtxMenuForUnFavorite?: LongPressContextMenuItem;
}
interface ProxyNode_Params {
    item?: ProxyItem;
    disabled?: boolean;
    selected?: string;
    selectProxy?: (p: ProxyItem) => void;
    onLatencyChange?: (p: number) => void;
}
import hilog from "@ohos:hilog";
import { LengthMetrics } from "@ohos:arkui.node";
import { SymbolGlyphModifier } from "@ohos:arkui.modifier";
import webview from "@ohos:web.webview";
// --- stubs for the project-local imports (see README.md) ---
// ../../common/entity/Constants
const BORDER_RADIUS_20: number = 20;
const CONTENT_END_OFFSET_150: number = 150;
const LIST_ITEM_HEIGHT: number = 64;
const TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE: string = '16fp';
// ../../common/datasources/ProxyData
class ProxyItem {
    name: string = '';
    latency: number = 0;
    type: string = '';
    id: string = '';
    g: string = '';
    isShowFavoriteProxy: boolean = false;
}
class ProxyItemDataSource implements IDataSource {
    private listeners: DataChangeListener[] = [];
    private items: ProxyItem[] = [];
    totalCount(): number {
        return this.items.length;
    }
    getData(index: number): ProxyItem {
        return this.items[index];
    }
    registerDataChangeListener(listener: DataChangeListener): void {
        if (this.listeners.indexOf(listener) < 0) {
            this.listeners.push(listener);
        }
    }
    unregisterDataChangeListener(listener: DataChangeListener): void {
        const pos = this.listeners.indexOf(listener);
        if (pos >= 0) {
            this.listeners.splice(pos, 1);
        }
    }
    clear(): void {
        this.items = [];
    }
    DefaultSortItems(items: ProxyItem[]): void {
        this.items = items.slice();
    }
    NameSortItems(items: ProxyItem[]): void {
        this.items = items.slice();
    }
    LatencySortItems(items: ProxyItem[]): void {
        this.items = items.slice();
    }
}
class ProxyGroupItemDataSource {
    getAllData(): ProxyItem[] {
        return [];
    }
}
// proxy_core
interface Profile {
    id: string;
    name: string;
}
enum ProxySort {
    Default = 0,
    Title = 1,
    Delay = 2
}
// ../../entryability/AppState
enum ProxyCardSize {
    Small = 0,
    Large = 1
}
enum ProxyGroupType {
    List = 0,
    Grid = 1
}
enum ClashCore {
    clash = 0,
    mihomo = 1
}
class AppConfig {
    proxySort: ProxySort = ProxySort.Default;
    currentProfileId: string | null = null;
    clashCore: ClashCore = ClashCore.clash;
    proxyGroupType: ProxyGroupType = ProxyGroupType.List;
}
class UIConfig {
    isVibrate: boolean = true;
    isAnimation: boolean = true;
    ShowProxyPop: boolean = false;
}
class AppState {
    static fetchFavoriteProxys(favorites: Map<string, string>, items: ProxyItem[]): string[] {
        return [];
    }
}
// ../../common/utils/Animation
class customAnimationUtil {
    static isScaleTranFast(index: number, config: UIConfig): TransitionEffect {
        return TransitionEffect.OPACITY.animation({ duration: 100 });
    }
    static isClickEffect(config: UIConfig): ClickEffect {
        return { level: ClickEffectLevel.LIGHT };
    }
}
// ../../common/utils/VibratorUtil
class customVibrator {
    static vibratorTriggerOfHapticClockTimer(): void {
    }
}
// ../../entryability/ClashViewModel
class ClashViewModel {
    static testDelay(name: string): void {
    }
}
// ../Start/Popup
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
// ../../common/breakpoint/BreakPoint
function ListGutter(widthBp: number, heightBp: number): Dimension {
    return 0;
}
function ProxyListLanes(widthBp: number, heightBp: number): number {
    return 1;
}
// xb_components
interface ToastOptions {
    message: ResourceStr;
}
class Xb_ToastUtil {
    static showToast(options: ToastOptions): void {
    }
}
class ProxyNode extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__item = new SynchedPropertyObjectOneWayPU(params.item, this, "item");
        this.__disabled = new SynchedPropertySimpleOneWayPU(params.disabled, this, "disabled");
        this.__selected = new SynchedPropertySimpleOneWayPU(params.selected, this, "selected");
        this.selectProxy = () => {
        };
        this.onLatencyChange = () => {
        };
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ProxyNode_Params) {
        if (params.selectProxy !== undefined) {
            this.selectProxy = params.selectProxy;
        }
        if (params.onLatencyChange !== undefined) {
            this.onLatencyChange = params.onLatencyChange;
        }
    }
    updateStateVars(params: ProxyNode_Params) {
        this.__item.reset(params.item);
        this.__disabled.reset(params.disabled);
        this.__selected.reset(params.selected);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__item.purgeDependencyOnElmtId(rmElmtId);
        this.__disabled.purgeDependencyOnElmtId(rmElmtId);
        this.__selected.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__item.aboutToBeDeleted();
        this.__disabled.aboutToBeDeleted();
        this.__selected.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private __item: SynchedPropertySimpleOneWayPU<ProxyItem>;
    get item() {
        return this.__item.get();
    }
    set item(newValue: ProxyItem) {
        this.__item.set(newValue);
    }
    private __disabled: SynchedPropertySimpleOneWayPU<boolean>;
    get disabled() {
        return this.__disabled.get();
    }
    set disabled(newValue: boolean) {
        this.__disabled.set(newValue);
    }
    private __selected: SynchedPropertySimpleOneWayPU<string>;
    get selected() {
        return this.__selected.get();
    }
    set selected(newValue: string) {
        this.__selected.set(newValue);
    }
    private selectProxy: (p: ProxyItem) => void;
    private onLatencyChange: (p: number) => void;
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
// --- original file follows verbatim ---
// 组件名
const componentName: string = 'ProxyPage';
// 长按菜单图标
const startIconModifier1: SymbolGlyphModifier = new SymbolGlyphModifier({ "id": 125832654, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }).fontSize('24vp');
const startIconModifier2: SymbolGlyphModifier = new SymbolGlyphModifier({ "id": 125832304, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }).fontSize('24vp');
const startIconModifier3: SymbolGlyphModifier = new SymbolGlyphModifier({ "id": 125832644, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }).fontSize('24vp');
class ProxyGroupItem extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__icon_emphasize = this.initializeConsume('icon_emphasize', "icon_emphasize");
        this.__currentTouchIndex = new ObservedPropertySimplePU(0
        // 主界面index
        , this, "currentTouchIndex");
        this.__tabCurrentPage = this.initializeConsume("tabCurrentPage", "tabCurrentPage");
        this.__ProxyGroupItemSelected = new SynchedPropertySimpleOneWayPU(params.ProxyGroupItemSelected, this, "ProxyGroupItemSelected");
        this.__isShowNetworkSpeed = new ObservedPropertySimplePU(false, this, "isShowNetworkSpeed");
        this.__isShowMenu = new ObservedPropertySimplePU(false
        //网络测速半模拟态web控制器
        , this, "isShowMenu");
        this.controller = new webview.WebviewController();
        this.__isEnableIndexForegroundBlur = this.initializeConsume('isEnableIndexForegroundBlur', "isEnableIndexForegroundBlur");
        this.__needChangeSort = this.createStorageLink('SortChange', false
        /* 代理节点数据相关参数  */
        // 节点数据组
        , "needChangeSort");
        this.__proxyitems = new SynchedPropertyObjectOneWayPU(params.proxyitems, this, "proxyitems");
        this.__proxyGroup = new SynchedPropertySimpleOneWayPU(params.proxyGroup, this, "proxyGroup");
        this.__proxyGroups = this.initializeConsume("proxyGroups", "proxyGroups");
        this.__proxyLazyItems = new ObservedPropertyObjectPU(new ProxyItemDataSource(), this, "proxyLazyItems");
        this.__currentProfile = this.initializeConsume('currentProfile', "currentProfile");
        this.__selectedProxy = new SynchedPropertySimpleOneWayPU(params.selectedProxy, this, "selectedProxy");
        this.__disabled = new SynchedPropertySimpleOneWayPU(params.disabled, this, "disabled");
        this.__widthBp = this.createStorageLink('currentWidthBreakpoint', 1
        // 高度断点
        , "widthBp");
        this.__heightBp = this.createStorageLink('currentHeightBreakpoint', 0, "heightBp");
        this.__favoriteProxys = this.createStorageLink('favoriteProxys', new Map<string, string>([]), "favoriteProxys");
        this.__appConfig = this.createStorageLink('appConfig', new AppConfig(), "appConfig");
        this.__uiConfig = this.createStorageLink('uiConfig', new UIConfig()
        // 收藏显示判定
        , "uiConfig");
        this.__showStart = this.initializeConsume("showStart", "showStart");
        this.__isShowDelayPop = this.initializeConsume("isShowDelayPop", "isShowDelayPop");
        this.__isRolling = this.initializeConsume("isRolling", "isRolling");
        this.__curOffsetRemain = this.initializeConsume("curOffsetRemain", "curOffsetRemain");
        this.__isPXSecScreen = this.initializeConsume("isPXSecScreen", "isPXSecScreen");
        this.ProxyDialogController = new CustomDialogController({
            builder: () => {
                let jsDialog = new ProxyCustomDialog(this, {
                    cancel: () => {
                        this.onCancel();
                    },
                    confirm: () => {
                        this.onAccept();
                    },
                }, undefined, -1, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 368, col: 14 });
                jsDialog.setController(this.
                /** 速度与延迟区别弹窗 */
                ProxyDialogController);
                ViewPU.create(jsDialog);
                let paramsLambda = () => {
                    return {
                        cancel: () => {
                            this.onCancel();
                        },
                        confirm: () => {
                            this.onAccept();
                        }
                    };
                };
                jsDialog.paramsGenerator_ = paramsLambda;
            },
            alignment: DialogAlignment.Center,
            customStyle: false,
            cornerRadius: 32,
            gridCount: 4,
            cancel: this.existApp,
            autoCancel: true,
            onWillDismiss: (dismissDialogAction: DismissDialogAction) => {
                console.info("reason=" + JSON.stringify(dismissDialogAction.reason));
                console.log("dialog onWillDismiss");
                if (dismissDialogAction.reason == DismissReason.PRESS_BACK) {
                    dismissDialogAction.dismiss();
                    this.isEnableIndexForegroundBlur = false;
                }
                if (dismissDialogAction.reason == DismissReason.TOUCH_OUTSIDE) {
                    dismissDialogAction.dismiss();
                    this.isEnableIndexForegroundBlur = false;
                }
            }
        }, this);
        this.OnProxyChange = () => {
        };
        this.proxyListLongPressCtxMenuForFavorite = {
            symbolStartIcon: new SymbolGlyphModifier({ "id": 125831520, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" })
                .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE).fontColor([{ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }]),
            content: '收藏', callback: () => {
                this.favoriteProxy();
            }
        };
        this.proxyListLongPressCtxMenuForUnFavorite = {
            symbolStartIcon: new SymbolGlyphModifier({ "id": 125831521, "type": 40000, params: [], "bundleName": "com.example.demo", "moduleName": "entry" })
                .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE).fontColor([{ "id": 125831041, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }]),
            content: '取消收藏', callback: () => {
                this.cancelFavoriteProxy();
            }
        };
        this.setInitiallyProvidedValue(params);
        this.declareWatch("needChangeSort", this.onSortChange);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ProxyGroupItem_Params) {
        if (params.currentTouchIndex !== undefined) {
            this.currentTouchIndex = params.currentTouchIndex;
        }
        if (params.isShowNetworkSpeed !== undefined) {
            this.isShowNetworkSpeed = params.isShowNetworkSpeed;
        }
        if (params.isShowMenu !== undefined) {
            this.isShowMenu = params.isShowMenu;
        }
        if (params.controller !== undefined) {
            this.controller = params.controller;
        }
        if (params.proxyLazyItems !== undefined) {
            this.proxyLazyItems = params.proxyLazyItems;
        }
        if (params.ProxyDialogController !== undefined) {
            this.ProxyDialogController = params.ProxyDialogController;
        }
        if (params.OnProxyChange !== undefined) {
            this.OnProxyChange = params.OnProxyChange;
        }
        if (params.proxyListLongPressCtxMenuForFavorite !== undefined) {
            this.proxyListLongPressCtxMenuForFavorite = params.proxyListLongPressCtxMenuForFavorite;
        }
        if (params.proxyListLongPressCtxMenuForUnFavorite !== undefined) {
            this.proxyListLongPressCtxMenuForUnFavorite = params.proxyListLongPressCtxMenuForUnFavorite;
        }
    }
    updateStateVars(params: ProxyGroupItem_Params) {
        this.__ProxyGroupItemSelected.reset(params.ProxyGroupItemSelected);
        this.__proxyitems.reset(params.proxyitems);
        this.__proxyGroup.reset(params.proxyGroup);
        this.__selectedProxy.reset(params.selectedProxy);
        this.__disabled.reset(params.disabled);
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__icon_emphasize.purgeDependencyOnElmtId(rmElmtId);
        this.__currentTouchIndex.purgeDependencyOnElmtId(rmElmtId);
        this.__tabCurrentPage.purgeDependencyOnElmtId(rmElmtId);
        this.__ProxyGroupItemSelected.purgeDependencyOnElmtId(rmElmtId);
        this.__isShowNetworkSpeed.purgeDependencyOnElmtId(rmElmtId);
        this.__isShowMenu.purgeDependencyOnElmtId(rmElmtId);
        this.__isEnableIndexForegroundBlur.purgeDependencyOnElmtId(rmElmtId);
        this.__needChangeSort.purgeDependencyOnElmtId(rmElmtId);
        this.__proxyitems.purgeDependencyOnElmtId(rmElmtId);
        this.__proxyGroup.purgeDependencyOnElmtId(rmElmtId);
        this.__proxyGroups.purgeDependencyOnElmtId(rmElmtId);
        this.__proxyLazyItems.purgeDependencyOnElmtId(rmElmtId);
        this.__currentProfile.purgeDependencyOnElmtId(rmElmtId);
        this.__selectedProxy.purgeDependencyOnElmtId(rmElmtId);
        this.__disabled.purgeDependencyOnElmtId(rmElmtId);
        this.__widthBp.purgeDependencyOnElmtId(rmElmtId);
        this.__heightBp.purgeDependencyOnElmtId(rmElmtId);
        this.__favoriteProxys.purgeDependencyOnElmtId(rmElmtId);
        this.__appConfig.purgeDependencyOnElmtId(rmElmtId);
        this.__uiConfig.purgeDependencyOnElmtId(rmElmtId);
        this.__showStart.purgeDependencyOnElmtId(rmElmtId);
        this.__isShowDelayPop.purgeDependencyOnElmtId(rmElmtId);
        this.__isRolling.purgeDependencyOnElmtId(rmElmtId);
        this.__curOffsetRemain.purgeDependencyOnElmtId(rmElmtId);
        this.__isPXSecScreen.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__icon_emphasize.aboutToBeDeleted();
        this.__currentTouchIndex.aboutToBeDeleted();
        this.__tabCurrentPage.aboutToBeDeleted();
        this.__ProxyGroupItemSelected.aboutToBeDeleted();
        this.__isShowNetworkSpeed.aboutToBeDeleted();
        this.__isShowMenu.aboutToBeDeleted();
        this.__isEnableIndexForegroundBlur.aboutToBeDeleted();
        this.__needChangeSort.aboutToBeDeleted();
        this.__proxyitems.aboutToBeDeleted();
        this.__proxyGroup.aboutToBeDeleted();
        this.__proxyGroups.aboutToBeDeleted();
        this.__proxyLazyItems.aboutToBeDeleted();
        this.__currentProfile.aboutToBeDeleted();
        this.__selectedProxy.aboutToBeDeleted();
        this.__disabled.aboutToBeDeleted();
        this.__widthBp.aboutToBeDeleted();
        this.__heightBp.aboutToBeDeleted();
        this.__favoriteProxys.aboutToBeDeleted();
        this.__appConfig.aboutToBeDeleted();
        this.__uiConfig.aboutToBeDeleted();
        this.__showStart.aboutToBeDeleted();
        this.__isShowDelayPop.aboutToBeDeleted();
        this.__isRolling.aboutToBeDeleted();
        this.__curOffsetRemain.aboutToBeDeleted();
        this.__isPXSecScreen.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    /* 主题色 */
    private __icon_emphasize: ObservedPropertyAbstractPU<ResourceColor>;
    get icon_emphasize() {
        return this.__icon_emphasize.get();
    }
    set icon_emphasize(newValue: ResourceColor) {
        this.__icon_emphasize.set(newValue);
    }
    private __currentTouchIndex: ObservedPropertySimplePU<number>;
    get currentTouchIndex() {
        return this.__currentTouchIndex.get();
    }
    set currentTouchIndex(newValue: number) {
        this.__currentTouchIndex.set(newValue);
    }
    // 主界面index
    private __tabCurrentPage: ObservedPropertyAbstractPU<number
    /* 本页布局相关参数 */
    // 定义ProxyGroupItemSelectedWide的值便于切换布局方式
    >;
    get tabCurrentPage() {
        return this.__tabCurrentPage.get();
    }
    set tabCurrentPage(newValue: number) {
        this.__tabCurrentPage.set(newValue);
    }
    /* 本页布局相关参数 */
    // 定义ProxyGroupItemSelectedWide的值便于切换布局方式
    private __ProxyGroupItemSelected: SynchedPropertySimpleOneWayPU<ProxyCardSize
    //是否显示网络测速半模拟态判定
    >;
    get ProxyGroupItemSelected() {
        return this.__ProxyGroupItemSelected.get();
    }
    set ProxyGroupItemSelected(newValue: ProxyCardSize) {
        this.__ProxyGroupItemSelected.set(newValue);
    }
    //是否显示网络测速半模拟态判定
    private __isShowNetworkSpeed: ObservedPropertySimplePU<boolean>;
    get isShowNetworkSpeed() {
        return this.__isShowNetworkSpeed.get();
    }
    set isShowNetworkSpeed(newValue: boolean) {
        this.__isShowNetworkSpeed.set(newValue);
    }
    private __isShowMenu: ObservedPropertySimplePU<boolean>;
    get isShowMenu() {
        return this.__isShowMenu.get();
    }
    set isShowMenu(newValue: boolean) {
        this.__isShowMenu.set(newValue);
    }
    //网络测速半模拟态web控制器
    private controller: webview.WebviewController;
    /* 是否开启 `Index` 页面的前景模糊 */
    private __isEnableIndexForegroundBlur: ObservedPropertyAbstractPU<boolean
    // 是否需要改变排列顺序
    >;
    get isEnableIndexForegroundBlur() {
        return this.__isEnableIndexForegroundBlur.get();
    }
    set isEnableIndexForegroundBlur(newValue: boolean) {
        this.__isEnableIndexForegroundBlur.set(newValue);
    }
    // 是否需要改变排列顺序
    private __needChangeSort: ObservedPropertyAbstractPU<boolean>;
    get needChangeSort() {
        return this.__needChangeSort.get();
    }
    set needChangeSort(newValue: boolean) {
        this.__needChangeSort.set(newValue);
    }
    /* 代理节点数据相关参数  */
    // 节点数据组
    private __proxyitems: SynchedPropertySimpleOneWayPU<ProxyItem[]
    // 代理分组序号[0 ~ +∞]
    >;
    get proxyitems() {
        return this.__proxyitems.get();
    }
    set proxyitems(newValue: ProxyItem[]) {
        this.__proxyitems.set(newValue);
    }
    // 代理分组序号[0 ~ +∞]
    private __proxyGroup: SynchedPropertySimpleOneWayPU<number
    // 代理数据(懒加载)
    >;
    get proxyGroup() {
        return this.__proxyGroup.get();
    }
    set proxyGroup(newValue: number) {
        this.__proxyGroup.set(newValue);
    }
    // 代理数据(懒加载)
    private __proxyGroups: ObservedPropertyAbstractPU<ProxyGroupItemDataSource>;
    get proxyGroups() {
        return this.__proxyGroups.get();
    }
    set proxyGroups(newValue: ProxyGroupItemDataSource) {
        this.__proxyGroups.set(newValue);
    }
    private __proxyLazyItems: ObservedPropertyObjectPU<ProxyItemDataSource>;
    get proxyLazyItems() {
        return this.__proxyLazyItems.get();
    }
    set proxyLazyItems(newValue: ProxyItemDataSource) {
        this.__proxyLazyItems.set(newValue);
    }
    private __currentProfile: ObservedPropertyAbstractPU<Profile | null
    // 当前代理节点单选选择的值
    >;
    get currentProfile() {
        return this.__currentProfile.get();
    }
    set currentProfile(newValue: Profile | null) {
        this.__currentProfile.set(newValue);
    }
    // 当前代理节点单选选择的值
    private __selectedProxy: SynchedPropertySimpleOneWayPU<string>;
    get selectedProxy() {
        return this.__selectedProxy.get();
    }
    set selectedProxy(newValue: string) {
        this.__selectedProxy.set(newValue);
    }
    private __disabled: SynchedPropertySimpleOneWayPU<boolean
    // 宽度断点
    >;
    get disabled() {
        return this.__disabled.get();
    }
    set disabled(newValue: boolean) {
        this.__disabled.set(newValue);
    }
    // 宽度断点
    private __widthBp: ObservedPropertyAbstractPU<WidthBreakpoint>;
    get widthBp() {
        return this.__widthBp.get();
    }
    set widthBp(newValue: WidthBreakpoint) {
        this.__widthBp.set(newValue);
    }
    // 高度断点
    private __heightBp: ObservedPropertyAbstractPU<HeightBreakpoint>;
    get heightBp() {
        return this.__heightBp.get();
    }
    set heightBp(newValue: HeightBreakpoint) {
        this.__heightBp.set(newValue);
    }
    private __favoriteProxys: ObservedPropertyAbstractPU<Map<string, string>>;
    get favoriteProxys() {
        return this.__favoriteProxys.get();
    }
    set favoriteProxys(newValue: Map<string, string>) {
        this.__favoriteProxys.set(newValue);
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
    // 收藏显示判定
    private __showStart: ObservedPropertyAbstractPU<boolean
    // 代理页延迟按钮气泡提示
    >;
    get showStart() {
        return this.__showStart.get();
    }
    set showStart(newValue: boolean) {
        this.__showStart.set(newValue);
    }
    // 代理页延迟按钮气泡提示
    private __isShowDelayPop: ObservedPropertyAbstractPU<boolean
    // 列表滚动判断
    >;
    get isShowDelayPop() {
        return this.__isShowDelayPop.get();
    }
    set isShowDelayPop(newValue: boolean) {
        this.__isShowDelayPop.set(newValue);
    }
    // 列表滚动判断
    private __isRolling: ObservedPropertyAbstractPU<boolean
    // px外屏隐藏top和tab偏移
    >;
    get isRolling() {
        return this.__isRolling.get();
    }
    set isRolling(newValue: boolean) {
        this.__isRolling.set(newValue);
    }
    // px外屏隐藏top和tab偏移
    private __curOffsetRemain: ObservedPropertyAbstractPU<number
    // px外屏
    >;
    get curOffsetRemain() {
        return this.__curOffsetRemain.get();
    }
    set curOffsetRemain(newValue: number) {
        this.__curOffsetRemain.set(newValue);
    }
    // px外屏
    private __isPXSecScreen: ObservedPropertyAbstractPU<boolean
    /**
     * XXX 当前手指触摸的ListItem的对应 `this.configList` 中的 `configId`
     *    List的长按菜单中的响应都是根据这个的对象中的属性来处理的，是个不稳定因素
     */
    >;
    get isPXSecScreen() {
        return this.__isPXSecScreen.get();
    }
    set isPXSecScreen(newValue: boolean) {
        this.__isPXSecScreen.set(newValue);
    }
    /**
     * XXX 当前手指触摸的ListItem的对应 `this.configList` 中的 `configId`
     *    List的长按菜单中的响应都是根据这个的对象中的属性来处理的，是个不稳定因素
     */
    public static currentTouchProxyData: ProxyItem = {
        name: '',
        latency: 0,
        type: '',
        id: '',
        g: '',
        isShowFavoriteProxy: false
    };
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            List.create({ space: 10, initialIndex: 0 });
            List.cachedCount(4);
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
            List.lanes(this.ProxyGroupItemSelected === ProxyCardSize.Large ? 0 : ProxyListLanes(this.widthBp, this.heightBp), ListGutter(this.widthBp, this.heightBp));
            __List__ListStyle();
            List.nestedScroll({
                scrollForward: this.isPXSecScreen ? NestedScrollMode.SELF_FIRST : NestedScrollMode.PARENT_FIRST,
                scrollBackward: this.isPXSecScreen ? NestedScrollMode.SELF_FIRST : NestedScrollMode.SELF_FIRST
            });
            List.contentEndOffset(this.isPXSecScreen ? 65 : this.appConfig.proxyGroupType === ProxyGroupType.List ? 10 : CONTENT_END_OFFSET_150);
        }, List);
        {
            const __lazyForEachItemGenFunction = (_item, index: number) => {
                const item = _item;
                {
                    const itemCreation2 = (elmtId, isInitialRender) => {
                        ListItem.create(() => { }, false);
                        ListItem.transition(customAnimationUtil.isScaleTranFast(index, ObservedObject.GetRawObject(this.uiConfig)));
                        ListItem.onClick(() => {
                            this.selectProxy(item);
                        });
                        __ListItem__ListItemStyle();
                        ListItem.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
                        ListItem.height(LIST_ITEM_HEIGHT + (this.ProxyGroupItemSelected === ProxyCardSize.Small ? 32 : 0));
                        ListItem.bindContextMenu(this.isShowMenu && this.currentTouchIndex == index, this.isPXSecScreen ? null : { builder: () => {
                                this.MyMenu.call(this, item);
                            } }, {
                            preview: MenuPreviewMode.IMAGE,
                            placement: Placement.BottomRight,
                            aboutToAppear: () => {
                                // js 闭包 在函数里只能拿到最后一个item的值
                                // ProxyGroupItem.currentTouchProxyData = item
                                if (this.uiConfig.isVibrate) {
                                    customVibrator.vibratorTriggerOfHapticClockTimer();
                                }
                            },
                            previewAnimationOptions: {
                                scale: [1.0, 1.1],
                                transition: this.uiConfig.isAnimation ? TransitionEffect.prototype.animation({ tempo: 1.5 }) : undefined
                            },
                            aboutToDisappear: () => {
                                this.isShowMenu = false;
                            }
                        });
                        ListItem.onMouse((event) => {
                            if (event.button == MouseButton.Right) {
                                this.currentTouchIndex = index;
                                ProxyGroupItem.currentTouchProxyData = item;
                                this.isShowMenu = true;
                            }
                        });
                        globalThis.Gesture.create(GesturePriority.Low);
                        LongPressGesture.create();
                        LongPressGesture.onAction(() => {
                            this.currentTouchIndex = index;
                            ProxyGroupItem.currentTouchProxyData = item;
                            this.isShowMenu = true;
                        });
                        LongPressGesture.pop();
                        globalThis.Gesture.pop();
                        ListItem.bindPopup((this.uiConfig.ShowProxyPop && this.proxyGroup === 0 && index === 0 && this.tabCurrentPage === 1), {
                            builder: { builder: () => {
                                    PopupBuilder.call(this, this, PopTips[2], () => {
                                        this.uiConfig.ShowProxyPop = false;
                                        this.isShowDelayPop = true;
                                    });
                                } },
                            onWillDismiss: false,
                            autoCancel: false
                        });
                    };
                    const observedDeepRender = () => {
                        this.observeComponentCreation2(itemCreation2, ListItem);
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            __Common__.create();
                            __Common__.restoreId(this.getRestoreId(index, item.id));
                        }, __Common__);
                        {
                            this.observeComponentCreation2((elmtId, isInitialRender) => {
                                if (isInitialRender) {
                                    let componentCall = new ProxyNode(this, {
                                        item: item,
                                        disabled: this.disabled,
                                        selected: this.selectedProxy,
                                        selectProxy: (p: ProxyItem) => {
                                            this.selectProxy(p);
                                        },
                                        onLatencyChange: (p: number) => {
                                            item.latency = p;
                                            // hilog.info(0xFF00, "ProxyPageLatencyTag", "延迟运行3 延迟更新为%{public}d", p)
                                            // const nodeItem = this.theProxyGroups[this.proxyGroup]
                                            // nodeItem.proxies[index].latency = p
                                            // this.theProxyGroups.splice(this.proxyGroup, 1, nodeItem)
                                            // hilog.info(0xFF00, "ProxyPageLatencyTag", "延迟运行3 %{public}s", JSON.stringify(nodeItem))
                                        }
                                    }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 266, col: 11 });
                                    ViewPU.create(componentCall);
                                    let paramsLambda = () => {
                                        return {
                                            item: item,
                                            disabled: this.disabled,
                                            selected: this.selectedProxy,
                                            selectProxy: (p: ProxyItem) => {
                                                this.selectProxy(p);
                                            },
                                            onLatencyChange: (p: number) => {
                                                item.latency = p;
                                                // hilog.info(0xFF00, "ProxyPageLatencyTag", "延迟运行3 延迟更新为%{public}d", p)
                                                // const nodeItem = this.theProxyGroups[this.proxyGroup]
                                                // nodeItem.proxies[index].latency = p
                                                // this.theProxyGroups.splice(this.proxyGroup, 1, nodeItem)
                                                // hilog.info(0xFF00, "ProxyPageLatencyTag", "延迟运行3 %{public}s", JSON.stringify(nodeItem))
                                            }
                                        };
                                    };
                                    componentCall.paramsGenerator_ = paramsLambda;
                                }
                                else {
                                    this.updateStateVarsOfChildByElmtId(elmtId, {
                                        item: item,
                                        disabled: this.disabled,
                                        selected: this.selectedProxy
                                    });
                                }
                            }, { name: "ProxyNode" });
                        }
                        __Common__.pop();
                        ListItem.pop();
                    };
                    observedDeepRender();
                }
            };
            const __lazyForEachItemIdFunc = (item: ProxyItem, index: number) => this.getRestoreId(index, item.id).toString();
            LazyForEach.create("1", this, this.proxyLazyItems, __lazyForEachItemGenFunction, __lazyForEachItemIdFunc);
            LazyForEach.pop();
        }
        List.pop();
    }
    /** 速度与延迟区别弹窗 */
    private ProxyDialogController: CustomDialogController | null;
    async aboutToAppear(): Promise<void> {
        hilog.info(0xB000, componentName, `ProxySort #ProxyGroupItem 代理节点：%{public}s`, JSON.stringify(this.proxyitems));
        // 进入时排序
        this.onSortChange();
    }
    onSortChange() {
        if (!this.proxyitems) {
            console.error(`ProxySort 节点数据不存在`);
            return;
        }
        this.proxyLazyItems.clear();
        if (this.appConfig.proxySort == ProxySort.Default) {
            this.proxyLazyItems.DefaultSortItems(this.proxyitems);
        }
        else if (this.appConfig.proxySort == ProxySort.Title) {
            this.proxyLazyItems.NameSortItems(this.proxyitems);
        }
        else if (this.appConfig.proxySort == ProxySort.Delay) {
            this.proxyLazyItems.LatencySortItems(this.proxyitems);
            hilog.info(0xFF00, "ProxySort ProxyPageLatencyTag", "延迟排序成功 %{public}s", this.appConfig.proxySort);
        }
        console.log(`ProxySort 排序类型：${this.appConfig.proxySort}, 节点数量：${this.proxyLazyItems.totalCount()}个`);
        this.needChangeSort = false;
    }
    /** 将 字符串ID 转换为 HashID 用于 restoreId */
    getRestoreId(index: number, id?: string): number {
        if (id) {
            let hash = 0;
            for (let i = 0; i < id.length; i++) {
                const char = id.charCodeAt(i);
                hash = ((hash << 5) - hash) + char;
                hash |= 0; // Convert to 32bit integer
            }
            return hash;
        }
        return index;
    }
    /**
     * 半模态窗口测速网站
     */
    BindSheetNetworkSpeed(parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
            Row.width('100%');
            Row.height('100%');
            Row.alignItems(VerticalAlign.Center);
            Row.justifyContent(FlexAlign.Center);
            Row.backgroundColor(({ "id": 16777230, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }));
        }, Row);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Web.create({ src: 'https://speed.cloudflare.com', controller: this.controller });
            Web.javaScriptAccess(true);
            Web.onlineImageAccess(true);
            Web.imageAccess(true);
            Web.domStorageAccess(true);
        }, Web);
        Column.pop();
        Row.pop();
    }
    //半模态网络测速 end
    onCancel() {
        console.info('Callback when the first button is clicked');
    }
    onAccept() {
        console.info('Callback when the second button is clicked');
    }
    existApp() {
        console.info('Click the callback in the blank area');
    }
    private OnProxyChange: ((item: ProxyItem) => void);
    // 长按菜单 start
    // 长按菜单相关
    MyMenu(item: ProxyItem, parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Menu.create();
            Menu.menuItemDivider({ color: { "id": 16777231, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }, strokeWidth: LengthMetrics.vp(1) });
            Menu.backgroundColor({ "id": 16777230, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            Menu.borderRadius(BORDER_RADIUS_20);
            Menu.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
        }, Menu);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            // 根据当前配置信息来判断显示[收藏 /取消收藏] 单独处理 START
            if (this.favoriteProxys.has(ProxyGroupItem.currentTouchProxyData.g + '|' + ProxyGroupItem.currentTouchProxyData.name)) {
                this.ifElseBranchUpdateFunction(0, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        MenuItem.create({
                            symbolStartIcon: this.proxyListLongPressCtxMenuForUnFavorite.symbolStartIcon!
                                .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE),
                            content: this.proxyListLongPressCtxMenuForUnFavorite.content
                        });
                        MenuItem.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
                        MenuItem.contentFontColor({ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                        MenuItem.onClick(() => {
                            this.proxyListLongPressCtxMenuForUnFavorite.callback();
                        });
                    }, MenuItem);
                    MenuItem.pop();
                });
            }
            else {
                this.ifElseBranchUpdateFunction(1, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        MenuItem.create({
                            symbolStartIcon: this.proxyListLongPressCtxMenuForFavorite.symbolStartIcon!
                                .fontSize(TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE),
                            content: this.proxyListLongPressCtxMenuForFavorite.content
                        });
                        MenuItem.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
                        MenuItem.contentFontColor({ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
                        MenuItem.onClick(() => {
                            this.proxyListLongPressCtxMenuForFavorite.callback();
                        });
                    }, MenuItem);
                    MenuItem.pop();
                });
            }
        }, If);
        If.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            // ClashMeta内核时暂时不能测速及延迟  暂时先屏蔽
            if (this.appConfig.clashCore === ClashCore.mihomo) {
                this.ifElseBranchUpdateFunction(0, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        MenuItem.create({ symbolStartIcon: startIconModifier1, content: { "id": 16777226, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
                        MenuItem.onClick(() => {
                            ClashViewModel.testDelay(item.name);
                        });
                    }, MenuItem);
                    MenuItem.pop();
                });
            }
            // 暂时先屏蔽网站测速功能
            /* MenuItem({ symbolStartIcon: startIconModifier2, content: $r('app.string.Test_Speed') })
               .onClick(() => {
                 this.isShowNetworkSpeed = true
                 this.isEnableIndexForegroundBlur = true
               })
      
            MenuItem({ symbolStartIcon: startIconModifier3, content: $r('app.string.Delay_Speed') })
              .onClick(() => {
                if (this.ProxyDialogController != null) {
                  this.ProxyDialogController.open()
                  this.isEnableIndexForegroundBlur = true
                }
              })*/
            else //分割线
             {
                this.ifElseBranchUpdateFunction(1, () => {
                });
            }
        }, If);
        If.pop();
        Menu.pop();
    } // 长按菜单end
    selectProxy(item: ProxyItem) {
        if (!this.disabled) {
            // 选中态由内核成功事件统一发布，避免 RPC 尚未完成时显示假选中。
            this.OnProxyChange(item);
        }
    }
    public proxyListLongPressCtxMenuForFavorite: LongPressContextMenuItem;
    public proxyListLongPressCtxMenuForUnFavorite: LongPressContextMenuItem;
    /**
     * 收藏配置
     */
    private favoriteProxy(): void {
        // 收藏节点 & 判断是否已经收藏
        const p = ProxyGroupItem.currentTouchProxyData;
        console.debug(`FavoriteProxy [ProxyPage] 当前ProxyItem信息：${JSON.stringify(p)}`);
        if (AppState.fetchFavoriteProxys(this.favoriteProxys, this.proxyGroups.getAllData()).length < 6) {
            Xb_ToastUtil.showToast({ message: { "id": 16777220, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
            this.favoriteProxys.set(p.g + '|' + p.name, this.appConfig.currentProfileId ?? "");
            this.favoriteProxys.forEach((value, key) => {
                console.debug(`FavoriteProxy [ProxyPage] favoriteProxys收藏信息：${key}: ${value}`);
            });
        }
        else {
            Xb_ToastUtil.showToast({ message: { "id": 16777227, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
        }
    }
    /**
     * 取消收藏配置
     */
    private cancelFavoriteProxy(): void {
        Xb_ToastUtil.showToast({ message: { "id": 16777229, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" } });
        const p = ProxyGroupItem.currentTouchProxyData;
        this.favoriteProxys.delete(p.g + '|' + p.name);
        this.favoriteProxys.forEach((value, key) => {
            console.debug(`FavoriteProxy [ProxyPage] favoriteProxys收藏信息：${key}: ${value}`);
        });
    }
    rerender() {
        this.updateDirtyElements();
    }
}
class ProxyCustomDialog extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__isEnableIndexForegroundBlur = this.initializeConsume('isEnableIndexForegroundBlur', "isEnableIndexForegroundBlur");
        this.__icon_emphasize = this.initializeConsume('icon_emphasize', "icon_emphasize");
        this.controller = undefined;
        this.cancel = () => {
        };
        this.confirm = () => {
        };
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ProxyCustomDialog_Params) {
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
    updateStateVars(params: ProxyCustomDialog_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__isEnableIndexForegroundBlur.purgeDependencyOnElmtId(rmElmtId);
        this.__icon_emphasize.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__isEnableIndexForegroundBlur.aboutToBeDeleted();
        this.__icon_emphasize.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    /* 是否开启 `Index` 页面的前景模糊 */
    private __isEnableIndexForegroundBlur: ObservedPropertyAbstractPU<boolean
    /* 主题色 */
    >;
    get isEnableIndexForegroundBlur() {
        return this.__isEnableIndexForegroundBlur.get();
    }
    set isEnableIndexForegroundBlur(newValue: boolean) {
        this.__isEnableIndexForegroundBlur.set(newValue);
    }
    /* 主题色 */
    private __icon_emphasize: ObservedPropertyAbstractPU<ResourceColor>;
    get icon_emphasize() {
        return this.__icon_emphasize.get();
    }
    set icon_emphasize(newValue: ResourceColor) {
        this.__icon_emphasize.set(newValue);
    }
    private controller?: CustomDialogController;
    setController(ctr: CustomDialogController) {
        this.controller = ctr;
    }
    private cancel: () => void;
    private confirm: () => void;
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
            Column.backgroundColor({ "id": 16777230, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            Column.padding({
                left: 24,
                right: 24,
                top: 16,
                bottom: 16
            });
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create({ "id": 16777223, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            Text.fontSize(20);
            Text.padding({ bottom: 15 });
            Text.fontWeight(700);
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create({ "id": 16777224, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            Text.fontSize(16);
            Text.padding({ bottom: 8 });
        }, Text);
        Text.pop();
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Button.createWithLabel({ "id": 16777225, "type": 10003, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
            Button.fontSize(16);
            Button.width('100%');
            Button.onClick(() => {
                if (this.controller != undefined) {
                    this.controller.close();
                    this.isEnableIndexForegroundBlur = false;
                }
            });
            Button.backgroundColor(ObservedObject.GetRawObject(this.icon_emphasize));
        }, Button);
        Button.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
// 自定义弹窗内容end
/**
 * 长按菜单参数
 */
interface LongPressContextMenuItem {
    symbolStartIcon?: SymbolGlyphModifier;
    symbolEndIcon?: SymbolGlyphModifier;
    content: ResourceStr;
    callback: Callback<void>;
}
// list公用样式
function __List__ListStyle(): void {
    List.width('100%');
    List.chainAnimation(true);
    List.scrollBar(BarState.Off);
    List.edgeEffect(EdgeEffect.Spring);
    List.constraintSize({ minHeight: 120 });
    List.alignListItem(ListItemAlign.Center);
}
// ListItem公用样式
function __ListItem__ListItemStyle(): void {
    ListItem.width('100%');
    ListItem.borderRadius(BORDER_RADIUS_20);
    ListItem.backgroundColor({ "id": 16777230, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
}
export default ProxyGroupItem;
class ProxyGroupItemPage extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ProxyGroupItemPage_Params) {
    }
    updateStateVars(params: ProxyGroupItemPage_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
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
    static getEntryName(): string {
        return "ProxyGroupItemPage";
    }
}
registerNamedRoute(() => new ProxyGroupItemPage(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
