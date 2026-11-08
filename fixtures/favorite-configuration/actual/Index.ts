if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface FavoriteConfigurationPage_Params {}
interface FavoriteConfiguration_Params {
	favoriteProfiles?: Map<string, string>;
	heightBp?: HeightBreakpoint;
	widthBp?: WidthBreakpoint;
	appConfig?: AppConfig;
	uiConfig?: UIConfig;
	icon_emphasize?: ResourceColor;
	isShowHomeEdit?: boolean;
}
import { CardTitle } from '@normalized:N&&&entry/src/main/ets/components/Home/CardTitleBuilder&';
import { AppConfig, isEnd, UIConfig } from '@normalized:N&&&entry/src/main/ets/entryability/AppState&';
import { customVibrator } from '@normalized:N&&&entry/src/main/ets/common/utils/VibratorUtil&';
import { customAnimationUtil } from '@normalized:N&&&entry/src/main/ets/common/utils/Animation&';
import ClashViewModel from '@normalized:N&&&entry/src/main/ets/entryability/ClashViewModel&';
import { EventHub, EventKey } from '@normalized:N&&&entry/src/main/ets/common/EventHub&';
import { getResourceString } from '@normalized:N&&&entry/src/main/ets/common/utils/ResourceStringUtil&';
import { HomeCardDeleteButtonSize, HomeCardFontSize, HomeCardPadding, HomeCardPartSmallFontSize } from '@normalized:N&&&entry/src/main/ets/common/breakpoint/BreakPoint&';
import { Xb_ToastUtil } from '@normalized:N&&&entry/src/main/ets/common/utils/Toast&';
class FavoriteConfiguration extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.__favoriteProfiles = this.createStorageLink("favoriteProfiles", new Map<string, string>([])
        // 高度断点
        , "favoriteProfiles");
        this.__heightBp = this.createStorageLink("currentHeightBreakpoint", 0
        // 宽度断点
        , "heightBp");
        this.__widthBp = this.createStorageLink("currentWidthBreakpoint", 1, "widthBp");
        this.__appConfig = this.createStorageLink("appConfig", new AppConfig(), "appConfig");
        this.__uiConfig = this.createStorageLink("uiConfig", new UIConfig()
        /* 主题色 */
        , "uiConfig");
        this.__icon_emphasize = this.initializeConsume("icon_emphasize", "icon_emphasize");
        this.__isShowHomeEdit = this.initializeConsume("isShowHomeEdit", "isShowHomeEdit");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: FavoriteConfiguration_Params) {
    }
    updateStateVars(params: FavoriteConfiguration_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__favoriteProfiles.purgeDependencyOnElmtId(rmElmtId);
        this.__heightBp.purgeDependencyOnElmtId(rmElmtId);
        this.__widthBp.purgeDependencyOnElmtId(rmElmtId);
        this.__appConfig.purgeDependencyOnElmtId(rmElmtId);
        this.__uiConfig.purgeDependencyOnElmtId(rmElmtId);
        this.__icon_emphasize.purgeDependencyOnElmtId(rmElmtId);
        this.__isShowHomeEdit.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__favoriteProfiles.aboutToBeDeleted();
        this.__heightBp.aboutToBeDeleted();
        this.__widthBp.aboutToBeDeleted();
        this.__appConfig.aboutToBeDeleted();
        this.__uiConfig.aboutToBeDeleted();
        this.__icon_emphasize.aboutToBeDeleted();
        this.__isShowHomeEdit.aboutToBeDeleted();
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
    /* 主题色 */
    private __icon_emphasize: ObservedPropertyAbstractPU<ResourceColor
    // 编辑按钮点击后显示编辑状态判定
    >;
    get icon_emphasize() {
        return this.__icon_emphasize.get();
    }
    set icon_emphasize(newValue: ResourceColor) {
        this.__icon_emphasize.set(newValue);
    }
    // 编辑按钮点击后显示编辑状态判定
    private __isShowHomeEdit: ObservedPropertyAbstractPU<boolean>;
    get isShowHomeEdit() {
        return this.__isShowHomeEdit.get();
    }
    set isShowHomeEdit(newValue: boolean) {
        this.__isShowHomeEdit.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
            Column.padding(HomeCardPadding(this.widthBp, this.heightBp));
            Column.width('100%');
            Column.height('100%');
            Column.alignItems(HorizontalAlign.Start);
            Column.id('FavoriteConfigurationId');
        }, Column);
            {
                this.observeComponentCreation2((elmtId, isInitialRender) => {
                    if (isInitialRender) {
                        let componentCall = new CardTitle(this, { title: { "id": 16777222, "type": 10003, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' } }, undefined, elmtId, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 30, col: 7 });
                        ViewPU.create(componentCall);
                        let paramsLambda = () => {
                            return { title: { "id": 16777222, "type": 10003, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' } };
                        };
                        componentCall.paramsGenerator_ = paramsLambda;
                    }
                    else {
                        this.updateStateVarsOfChildByElmtId(elmtId, {});
                    }
                }, { name: "CardTitle" });
            }
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                If.create();
                // 收藏配置 start
                if (this.favoriteProfiles.size > 0) {
                    this.ifElseBranchUpdateFunction(0, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            List.create();
                            List.width('100%');
                            List.height('82%');
                            List.backgroundColor({ "id": 16777227, "type": 10001, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
                            List.borderRadius(10);
                            List.padding({ left: 8, right: 5 });
                        }, List);
                            this.observeComponentCreation2((elmtId, isInitialRender) => {
                                ForEach.create();
                                const forEachItemGenFunction = (_item, i: number) => {
                                    const f = _item;
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
                                            ListItem.height(this.favoriteProfiles.size - 1 == 1 ? '50%' : '32%');
                                            ListItem.onClick(async () => {
              if (!this.isShowHomeEdit) {
                this.appConfig.currentProfileId = f[0]
                EventHub.sendEvent(EventKey.LoadClashConfig)
                await ClashViewModel.loadConfig(false, 'FavoriteConfiguration-点击收藏行');
                Xb_ToastUtil.showToast({message: getResourceString({ "id": 16777224, "type": 10003, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' }, this)})
                if (this.uiConfig.isVibrate) {
                  customVibrator.vibratorTriggerOfHapticClockTimer()
                }
              }
            });
                                            ListItem.clickEffect(customAnimationUtil.isClickEffect(ObservedObject.GetRawObject(this.uiConfig)));
                                            ListItem.transition({ type: TransitionType.Delete, opacity: 0 });
                                            ListItem.swipeAction({
              end: {
                builder: () => {
                  this.itemEnd.bind(this)(f[0]);
                },
                actionAreaDistance: 56,
              },
              edgeEffect: SwipeEdgeEffect.Spring
            });
                                        };
                                        const deepRenderFunction = (elmtId, isInitialRender) => {
                                            itemCreation(elmtId, isInitialRender);
                                            this.observeComponentCreation2((elmtId, isInitialRender) => {
                                                Flex.create({ direction: FlexDirection.Row, alignItems: ItemAlign.Center, justifyContent: FlexAlign.SpaceEvenly });
                                            }, Flex);
                                                this.observeComponentCreation2((elmtId, isInitialRender) => {
                                                    Text.create(f[1]);
                                                    Text.fontSize(HomeCardFontSize(this.widthBp, this.heightBp));
                                                    Text.fontColor({ "id": 125830982, "type": 10001, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
                                                    Text.maxLines(1);
                                                    Text.width('80%');
                                                    Text.textOverflow({ overflow: TextOverflow.Ellipsis });
                                                }, Text);
                                                Text.pop();
                                                this.observeComponentCreation2((elmtId, isInitialRender) => {
                                                    Radio.create({ value: 'FavoriteConfigurationRadio1', group: 'FavoriteConfigurationRadioGroup' });
                                                    Radio.width(HomeCardFontSize(this.widthBp, this.heightBp));
                                                    Radio.checked(f[0] === this.appConfig.currentProfileId);
                                                    Radio.radioStyle({
                    checkedBackgroundColor: this.icon_emphasize,
                  });
                                                    Radio.hitTestBehavior(HitTestMode.None);
                                                }, Radio);
                                            Flex.pop();
                                            ListItem.pop();
                                        };
                                        this.observeComponentCreation2(itemCreation2, ListItem);
                                        ListItem.pop();
                                        }
                                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                                        If.create();
                                        if (!isEnd(this.favoriteProfiles.size, i)) {
                                            this.ifElseBranchUpdateFunction(0, () => {
                                                this.observeComponentCreation2((elmtId, isInitialRender) => {
                                                    Divider.create();
                                                    Divider.strokeWidth(1);
                                                    Divider.color({ "id": 16777226, "type": 10001, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
                                                }, Divider);
                                            });
                                        }
                                        else {
                                            this.ifElseBranchUpdateFunction(1, () => {
                                            });
                                        }
                                    }, If);
                                    If.pop();
                                };
                                this.forEachUpdateFunction(elmtId, Array.from(this.favoriteProfiles.entries()), forEachItemGenFunction, undefined, true, false);
                            }, ForEach);
                            ForEach.pop();
                        List.pop();
                    });
                }
                // 收藏配置 end
                // 无收藏配置时的布局 start
                else {
                    this.ifElseBranchUpdateFunction(1, () => {
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Flex.create({ direction: FlexDirection.Column, alignItems: ItemAlign.Center, justifyContent: FlexAlign.Center });
                            Flex.width('100%');
                            Flex.height("78%");
                        }, Flex);
                            this.observeComponentCreation2((elmtId, isInitialRender) => {
                                SymbolGlyph.create({ "id": 125831520, "type": 40000, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
                                SymbolGlyph.fontSize(HomeCardDeleteButtonSize(this.widthBp, this.heightBp));
                                SymbolGlyph.fontColor([{ "id": 125830983, "type": 10001, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' }]);
                                SymbolGlyph.padding({bottom:5});
                            }, SymbolGlyph);
                            this.observeComponentCreation2((elmtId, isInitialRender) => {
                                Text.create({ "id": 16777223, "type": 10003, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
                                Text.fontSize(HomeCardPartSmallFontSize(this.widthBp, this.heightBp));
                                Text.fontColor({ "id": 125830983, "type": 10001, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
                                Text.textAlign(TextAlign.Center);
                            }, Text);
                            Text.pop();
                        Flex.pop();
                    });
                }
            }, If);
            If.pop();
        Column.pop();
    }
    itemEnd(id: string, parent = null) {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Button.createWithChild({ type: ButtonType.Circle });
            Button.padding(5);
            Button.backgroundColor(Color.Transparent);
            Button.onClick(() => {
      this.favoriteProfiles.delete(id)
    });
        }, Button);
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                SymbolGlyph.create({ "id": 125831543, "type": 40000, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
                SymbolGlyph.fontColor([Color.Red]);
                SymbolGlyph.fontSize(HomeCardFontSize(this.widthBp, this.heightBp));
            }, SymbolGlyph);
        Button.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
}
export default FavoriteConfiguration;
class FavoriteConfigurationPage extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: FavoriteConfigurationPage_Params) {}
	updateStateVars(params: FavoriteConfigurationPage_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {}
	aboutToBeDeleted() {
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		{
			this.observeComponentCreation2((elmtId, isInitialRender) => {
				if (isInitialRender) {
					let componentCall = new FavoriteConfiguration(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 129,
						col: 7
					});
					ViewPU.create(componentCall);
					let paramsLambda = () => {
						return {};
					};
					componentCall.paramsGenerator_ = paramsLambda;
				} else {
					this.updateStateVarsOfChildByElmtId(elmtId, {});
				}
			}, { name: 'FavoriteConfiguration' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'FavoriteConfigurationPage';
	}
}
registerNamedRoute(() => new FavoriteConfigurationPage(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
