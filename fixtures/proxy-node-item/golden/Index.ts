if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => { });
}
interface ProxyNodeItemPage_Params {
}
interface ProxyNode_Params {
    icon_emphasize?: ResourceColor;
    uiConfig?: UIConfig;
    item?: ProxyItem;
    disabled?: boolean;
    delay?: number;
    selected?: string;
    appConfig?: AppConfig;
    isPXSecScreen?: boolean;
    selectProxy?: (item: ProxyItem) => void;
    onLatencyChange?: (l: number) => void;
}
import hilog from "@ohos:hilog";
// --- stubs for the project-local imports (see README.md) ---
enum EventKey {
    TestDelay = "TestDelay"
}
enum ProxyCardSize {
    Small = 0,
    Large = 1
}
class DelayInfo {
    name: string = '';
    delay: number = 0;
}
class ProxyItem {
    name: string = '';
    type: string = '';
    g: string = '';
}
class AppConfig {
    proxyCardSize: ProxyCardSize = ProxyCardSize.Small;
}
class UIConfig {
    stringDisplayIndex: number = 0;
}
class EventHub {
    static on(key: EventKey, callback: (delay: DelayInfo) => void, once: boolean): void {
    }
}
class ProxyData {
    static getColorForLatency(delay: number): ResourceColor {
        return Color.Black;
    }
}
class ClashViewModel {
    static delayMap: Map<string, DelayInfo> = new Map<string, DelayInfo>();
}
const customVibrator: number = 0;
export class ProxyNode extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__icon_emphasize = this.initializeConsume('icon_emphasize', "icon_emphasize");
        this.__uiConfig = this.createStorageLink('uiConfig', new UIConfig(), "uiConfig");
        this.__item = new SynchedPropertyObjectOneWayPU(params.item, this, "item");
        this.__disabled = new SynchedPropertySimpleOneWayPU(params.disabled, this, "disabled");
        this.__delay = new ObservedPropertySimplePU(0, this, "delay");
        this.__selected = new SynchedPropertySimpleOneWayPU(params.selected, this, "selected");
        this.__appConfig = this.createStorageLink('appConfig', new AppConfig(), "appConfig");
        this.__isPXSecScreen = this.initializeConsume("isPXSecScreen", "isPXSecScreen");
        this.selectProxy = undefined;
        this.onLatencyChange = undefined;
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ProxyNode_Params) {
        if (params.delay !== undefined) {
            this.delay = params.delay;
        }
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
        this.__icon_emphasize.purgeDependencyOnElmtId(rmElmtId);
        this.__uiConfig.purgeDependencyOnElmtId(rmElmtId);
        this.__item.purgeDependencyOnElmtId(rmElmtId);
        this.__disabled.purgeDependencyOnElmtId(rmElmtId);
        this.__delay.purgeDependencyOnElmtId(rmElmtId);
        this.__selected.purgeDependencyOnElmtId(rmElmtId);
        this.__appConfig.purgeDependencyOnElmtId(rmElmtId);
        this.__isPXSecScreen.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__icon_emphasize.aboutToBeDeleted();
        this.__uiConfig.aboutToBeDeleted();
        this.__item.aboutToBeDeleted();
        this.__disabled.aboutToBeDeleted();
        this.__delay.aboutToBeDeleted();
        this.__selected.aboutToBeDeleted();
        this.__appConfig.aboutToBeDeleted();
        this.__isPXSecScreen.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    updateRecycleElmtId(oldElmtId, newElmtId) {
        this.__icon_emphasize.updateElmtId(oldElmtId, newElmtId);
        this.__uiConfig.updateElmtId(oldElmtId, newElmtId);
        this.__item.updateElmtId(oldElmtId, newElmtId);
        this.__disabled.updateElmtId(oldElmtId, newElmtId);
        this.__delay.updateElmtId(oldElmtId, newElmtId);
        this.__selected.updateElmtId(oldElmtId, newElmtId);
        this.__appConfig.updateElmtId(oldElmtId, newElmtId);
        this.__isPXSecScreen.updateElmtId(oldElmtId, newElmtId);
    }
    private __icon_emphasize: ObservedPropertyAbstractPU<ResourceColor>;
    get icon_emphasize() {
        return this.__icon_emphasize.get();
    }
    set icon_emphasize(newValue: ResourceColor) {
        this.__icon_emphasize.set(newValue);
    }
    private __uiConfig: ObservedPropertyAbstractPU<UIConfig>;
    get uiConfig() {
        return this.__uiConfig.get();
    }
    set uiConfig(newValue: UIConfig) {
        this.__uiConfig.set(newValue);
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
    private __delay: ObservedPropertySimplePU<number>;
    get delay() {
        return this.__delay.get();
    }
    set delay(newValue: number) {
        this.__delay.set(newValue);
    }
    private __selected: SynchedPropertySimpleOneWayPU<string>;
    get selected() {
        return this.__selected.get();
    }
    set selected(newValue: string) {
        this.__selected.set(newValue);
    }
    private __appConfig: ObservedPropertyAbstractPU<AppConfig>;
    get appConfig() {
        return this.__appConfig.get();
    }
    set appConfig(newValue: AppConfig) {
        this.__appConfig.set(newValue);
    }
    private __isPXSecScreen: ObservedPropertyAbstractPU<boolean>;
    get isPXSecScreen() {
        return this.__isPXSecScreen.get();
    }
    set isPXSecScreen(newValue: boolean) {
        this.__isPXSecScreen.set(newValue);
    }
    private selectProxy?: (item: ProxyItem) => void;
    private onLatencyChange?: (l: number) => void;
    // 使用 @Reusable 必须实现 aboutToReuse 函数
    aboutToReuse(params: Record<string, Object>): void {
        this.item = params.item as ProxyItem;
        this.disabled = params.disabled as boolean;
        this.selected = params.selected as string;
        this.selectProxy = params.selectProxy as (item: ProxyItem) => void;
        this.onLatencyChange = params.onLatencyChange as (l: number) => void;
    }
    aboutToAppear(): void {
        this.delay = ClashViewModel.delayMap.get(this.item.name)?.delay ?? 0;
        EventHub.on(EventKey.TestDelay, async (delay: DelayInfo) => {
            if (delay.name == this.item.name) {
                this.delay = delay.delay;
            }
        }, false); // 批量监听
    }
    aboutToDisappear() {
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            If.create();
            if (this.appConfig.proxyCardSize === ProxyCardSize.Large) {
                this.ifElseBranchUpdateFunction(0, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Flex.create({ direction: FlexDirection.Row, alignItems: ItemAlign.Center, justifyContent: FlexAlign.SpaceBetween });
                        if (isInitialRender) {
                            Flex.width('100%');
                            Flex.padding({ left: 14, right: 16 });
                        }
                    }, Flex);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Column.create({ space: 3 });
                        if (isInitialRender) {
                            Column.layoutWeight(1);
                            Column.alignItems(HorizontalAlign.Start);
                        }
                    }, Column);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create(`${this.item.name}`);
                        __Text__ItemTextStyle();
                        if (isInitialRender) {
                            Text.maxLines(1);
                        }
                    }, Text);
                    Text.pop();
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create(`${this.item.type}`);
                        __Text__ItemTypeStyle();
                    }, Text);
                    Text.pop();
                    Column.pop();
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Row.create({ space: 5 });
                    }, Row);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create(delayText(this.delay));
                        Text.fontColor(ProxyData.getColorForLatency(this.delay));
                        if (isInitialRender) {
                            Text.fontSize(12);
                            Text.maxLines(1);
                            Text.textAlign(TextAlign.End);
                        }
                    }, Text);
                    Text.pop();
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        // group 不要写死, 会和其他tab页的生效，在一个组里唯一
                        Radio.create({ value: this.item.name, group: this.item.g + this.item.name });
                        // group 不要写死, 会和其他tab页的生效，在一个组里唯一
                        Radio.checked(this.item.name === this.selected);
                        // group 不要写死, 会和其他tab页的生效，在一个组里唯一
                        Radio.radioStyle({
                            checkedBackgroundColor: this.icon_emphasize,
                        });
                        if (isInitialRender) {
                            // group 不要写死, 会和其他tab页的生效，在一个组里唯一
                            Radio.hitTestBehavior(HitTestMode.None);
                        }
                    }, Radio);
                    Row.pop();
                    Flex.pop();
                });
            }
            else if (this.appConfig.proxyCardSize === ProxyCardSize.Small) {
                this.ifElseBranchUpdateFunction(1, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Row.create();
                        __Row__ListRowStyle();
                    }, Row);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Column.create();
                        if (isInitialRender) {
                            Column.layoutWeight(1);
                            Column.alignItems(HorizontalAlign.Start);
                            Column.justifyContent(FlexAlign.SpaceBetween);
                        }
                    }, Column);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create(`${this.item.name}`);
                        __Text__ItemTextStyle();
                        if (isInitialRender) {
                            Text.maxLines(2);
                            Text.padding({ bottom: 5 });
                        }
                    }, Text);
                    Text.pop();
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create(`${this.item.type}`);
                        __Text__ItemTypeStyle();
                        if (isInitialRender) {
                            Text.textOverflow({ overflow: TextOverflow.Ellipsis });
                        }
                    }, Text);
                    Text.pop();
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create(delayText(this.delay));
                        Text.fontColor(ProxyData.getColorForLatency(this.delay));
                        if (isInitialRender) {
                            Text.fontSize(12);
                            Text.maxLines(1);
                        }
                    }, Text);
                    Text.pop();
                    Column.pop();
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Radio.create({ value: this.item.name, group: this.item.g + this.item.name });
                        Radio.checked(this.item.name === this.selected);
                        Radio.radioStyle({
                            checkedBackgroundColor: this.icon_emphasize,
                        });
                        if (isInitialRender) {
                            Radio.hitTestBehavior(HitTestMode.None);
                        }
                    }, Radio);
                    Row.pop();
                });
            }
            else {
                this.ifElseBranchUpdateFunction(2, () => {
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Row.create();
                        __Row__ListRowStyle();
                    }, Row);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Column.create({ space: 3 });
                        if (isInitialRender) {
                            Column.layoutWeight(1);
                            Column.alignItems(HorizontalAlign.Start);
                        }
                    }, Column);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create(`${this.item.name}`);
                        __Text__ItemTextStyle();
                        if (isInitialRender) {
                            Text.maxLines(1);
                        }
                    }, Text);
                    Text.pop();
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Row.create({ space: 3 });
                        Row.width(this.uiConfig.stringDisplayIndex === 0 ? '70%' : '100%');
                        if (isInitialRender) {
                            Row.justifyContent(FlexAlign.Start);
                        }
                    }, Row);
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create(`${this.item.type}`);
                        __Text__ItemTypeStyle();
                        Text.fontSize(this.isPXSecScreen ? 10 : 12);
                        Text.constraintSize({ maxWidth: this.uiConfig.stringDisplayIndex === 0 ? '55%' : Infinity });
                        if (isInitialRender) {
                            Text.maxLines(1);
                            Text.textOverflow({ overflow: TextOverflow.Clip });
                        }
                    }, Text);
                    Text.pop();
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Text.create(`${delayText(this.delay)}`);
                        Text.fontColor(ProxyData.getColorForLatency(this.delay));
                        Text.textOverflow({ overflow: this.isPXSecScreen ? TextOverflow.MARQUEE : TextOverflow.Ellipsis });
                        Text.fontSize(this.isPXSecScreen ? 10 : 12);
                        Text.textAlign(this.uiConfig.stringDisplayIndex === 0 ? TextAlign.End : TextAlign.Start);
                        if (isInitialRender) {
                            Text.layoutWeight(1);
                            Text.maxLines(1);
                        }
                    }, Text);
                    Text.pop();
                    Row.pop();
                    Column.pop();
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        Radio.create({ value: this.item.name, group: this.item.g + this.item.name });
                        Radio.checked(this.item.name === this.selected);
                        Radio.radioStyle({
                            checkedBackgroundColor: this.icon_emphasize,
                        });
                        if (isInitialRender) {
                            Radio.hitTestBehavior(HitTestMode.None);
                            Radio.onChange((isChecked) => {
                                hilog.debug(0xFF00, "ProxyPageGroupTag", "ProxyNodeItem #onChange %{public}s的选中状态%{public}s", this.item.name, isChecked);
                            });
                        }
                    }, Radio);
                    Row.pop();
                });
            }
        }, If);
        If.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
function __Text__ItemTextStyle(): void {
    Text.fontColor(({ "id": 125830982, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" }));
    Text.fontSize(16);
    Text.fontWeight(FontWeight.Medium);
    Text.textOverflow({ overflow: TextOverflow.Ellipsis });
}
//Item中type（节点类型）公用样式
function __Text__ItemTypeStyle(): void {
    Text.fontColor({ "id": 125830983, "type": 10001, params: [], "bundleName": "com.example.demo", "moduleName": "entry" });
    Text.fontSize(12);
}
function __Row__ListRowStyle(): void {
    Row.width('100%');
    Row.padding({ left: 14, right: 8, top: 5, bottom: 5 });
}
export function delayText(delay: number) {
    if (delay == 0) {
        return "";
    }
    else if (delay < 0) {
        return "timeout";
    }
    else {
        return delay + "ms";
    }
}
class ProxyNodeItemPage extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: ProxyNodeItemPage_Params) {
    }
    updateStateVars(params: ProxyNodeItemPage_Params) {
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
        return "ProxyNodeItemPage";
    }
}
registerNamedRoute(() => new ProxyNodeItemPage(undefined, {}), "", { bundleName: "com.example.demo", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap" });
