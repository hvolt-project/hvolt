if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface CreateNewConfigPage_Params {}
interface CreateNewConfig_Params {
	componentName?: string;
	configAddPageInfos?: NavPathStack;
	heightBp?: HeightBreakpoint;
	widthBp?: WidthBreakpoint;
	mainTitle?: string;
	subTitle?: string;
}
import { BIND_SHEET_MAIN_FONT_WEIGHT, BIND_SHEET_MAIN_FONTSIZE, BIND_SHEET_SUB_FONTSIZE, BIND_SHEET_DRAG_BAR_HEIGHT, BIND_SHEET_TITLE_LEFT_SYMBOL_WEIGHT } from '@normalized:N&&&entry/src/main/ets/common/entity/Constants&';
import { BindSheetTitleCustomBuilder } from '@normalized:N&&&entry/src/main/ets/components/Common/Common&';
import hilog from '@ohos:hilog';
import { getResourceString } from '@normalized:N&&&entry/src/main/ets/common/utils/ResourceStringUtil&';
import { NavigationListPadding } from '@normalized:N&&&entry/src/main/ets/common/breakpoint/BreakPoint&';
export function CreateNewConfigBuilder(name: string, param: Object, parent = null) {
	{
		(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
			if (isInitialRender) {
				let componentCall = new CreateNewConfig(parent ? parent : this, {}, undefined, elmtId, () => {}, {
					page: 'entry/src/main/ets/pages/Index.ets',
					line: 19,
					col: 3
				});
				ViewPU.create(componentCall);
				let paramsLambda = () => {
					return {};
				};
				componentCall.paramsGenerator_ = paramsLambda;
			} else {
				(parent ? parent : this).updateStateVarsOfChildByElmtId(elmtId, {});
			}
		}, { name: 'CreateNewConfig' });
	}
}
export class CreateNewConfig extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.componentName = 'CreateNewConfig';
        this.__configAddPageInfos = this.initializeConsume("configAddPageInfos", "configAddPageInfos");
        this.__heightBp = this.createStorageLink("currentHeightBreakpoint", 0
        // 宽度断点
        , "heightBp");
        this.__widthBp = this.createStorageLink("currentWidthBreakpoint", 1
        // NV title Builder
        , "widthBp");
        this.__mainTitle = new ObservedPropertySimplePU(getResourceString({ "id": 16777222, "type": 10003, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' }, this), this, "mainTitle");
        this.__subTitle = new ObservedPropertySimplePU(getResourceString({ "id": 16777224, "type": 10003, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' }, this), this, "subTitle");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: CreateNewConfig_Params) {
        if (params.componentName !== undefined) {
            this.componentName = params.componentName;
        }
        if (params.mainTitle !== undefined) {
            this.mainTitle = params.mainTitle;
        }
        if (params.subTitle !== undefined) {
            this.subTitle = params.subTitle;
        }
    }
    updateStateVars(params: CreateNewConfig_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__configAddPageInfos.purgeDependencyOnElmtId(rmElmtId);
        this.__heightBp.purgeDependencyOnElmtId(rmElmtId);
        this.__widthBp.purgeDependencyOnElmtId(rmElmtId);
        this.__mainTitle.purgeDependencyOnElmtId(rmElmtId);
        this.__subTitle.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__configAddPageInfos.aboutToBeDeleted();
        this.__heightBp.aboutToBeDeleted();
        this.__widthBp.aboutToBeDeleted();
        this.__mainTitle.aboutToBeDeleted();
        this.__subTitle.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    private componentName: string;
    /* NV 路由相关 */
    // 页面栈
    private __configAddPageInfos: ObservedPropertyAbstractPU<NavPathStack
    // 高度断点
    >;
    get configAddPageInfos() {
        return this.__configAddPageInfos.get();
    }
    set configAddPageInfos(newValue: NavPathStack) {
        this.__configAddPageInfos.set(newValue);
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
    // NV title Builder
    private __mainTitle: ObservedPropertySimplePU<string>;
    get mainTitle() {
        return this.__mainTitle.get();
    }
    set mainTitle(newValue: string) {
        this.__mainTitle.set(newValue);
    }
    private __subTitle: ObservedPropertySimplePU<string>;
    get subTitle() {
        return this.__subTitle.get();
    }
    set subTitle(newValue: string) {
        this.__subTitle.set(newValue);
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            NavDestination.create(() => {
                    // 替代`NavDestination`的标题栏 START
                    BindSheetTitleCustomBuilder.bind(this)(makeBuilderParameterProxy("BindSheetTitleCustomBuilder", { pageInfos: () => (this["__configAddPageInfos"] ? this["__configAddPageInfos"] : this["configAddPageInfos"]), isSubPage: () => true, needSetHeight: () => true, titlePaddingLeft: () => BIND_SHEET_DRAG_BAR_HEIGHT / 2, mainTitle: () => (this["__mainTitle"] ? this["__mainTitle"] : this["mainTitle"]), subTitle: () => (this["__subTitle"] ? this["__subTitle"] : this["subTitle"]), mainTitleFontWeight: () => BIND_SHEET_MAIN_FONT_WEIGHT, mainTitleFontSize: () => BIND_SHEET_MAIN_FONTSIZE, subTitleFontSize: () => BIND_SHEET_SUB_FONTSIZE, titleBackgroundColor: () => ({ "id": 16777225, "type": 10001, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' }), padding: () => ({
          // 为什么设置`15`，看Builder的参数中`padding`的介绍：ConfigAddBindSheetTitleBuilderParam
          left: 15,
          right: BIND_SHEET_TITLE_LEFT_SYMBOL_WEIGHT + 15 * 2
        }), menus: () => [] }));
                    this.observeComponentCreation2((elmtId, isInitialRender) => {
                        // 替代`NavDestination`的标题栏 END
                        // START
                        Column.create();
                        // 替代`NavDestination`的标题栏 END
                        // START
                        Column.width('100%');
                        // 替代`NavDestination`的标题栏 END
                        // START
                        Column.height('auto');
                        // 替代`NavDestination`的标题栏 END
                        // START
                        Column.backgroundColor('#ffac70e9');
                        // 替代`NavDestination`的标题栏 END
                        // START
                        Column.alignItems(HorizontalAlign.Start);
                    }, Column);
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create(this.componentName);
                            Text.fontSize(50);
                        }, Text);
                        Text.pop();
                        this.observeComponentCreation2((elmtId, isInitialRender) => {
                            Text.create(`NavigationListPadding(this.widthBp, this.heightBp) value is [${NavigationListPadding(this.widthBp, this.heightBp)}]`);
                        }, Text);
                        Text.pop();
                    // 替代`NavDestination`的标题栏 END
                    // START
                    Column.pop();
            }, { moduleName: "entry", pagePath: "entry/src/main/ets/pages/Index" });
            NavDestination.height('100%');
            NavDestination.hideTitleBar(true);
            NavDestination.backgroundColor({ "id": 16777226, "type": 10001, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' });
        }, NavDestination);
        NavDestination.pop();
    }
aboutToAppear(): void {
    hilog.info(0xCD10, this.componentName, `#aboutToAppear executed`)
  }
aboutToDisappear(): void {
    hilog.info(0xCD10, this.componentName, `#aboutToDisappear executed`)
  }
    rerender() {
        this.updateDirtyElements();
    }
}
class CreateNewConfigPage extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: CreateNewConfigPage_Params) {}
	updateStateVars(params: CreateNewConfigPage_Params) {}
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
					let componentCall = new CreateNewConfig(this, {}, undefined, elmtId, () => {}, {
						page: 'entry/src/main/ets/pages/Index.ets',
						line: 98,
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
			}, { name: 'CreateNewConfig' });
		}
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'CreateNewConfigPage';
	}
}
registerNamedRoute(() => new CreateNewConfigPage(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
