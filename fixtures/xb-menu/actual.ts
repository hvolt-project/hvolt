if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface XbMenuPage_Params {
	selected?: number;
}
import type { MenuItem } from '../settings/SettingsCommonOp';
import { SymbolGlyphModifier } from '@ohos:arkui.modifier';
export function Xb_Menu(menuList: MenuItem[], onMenuSelect: (index: number) => void, isShowCheck: boolean, menuSelect?: string | number, parent = null) {
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
		Menu.create();
	}, Menu);
	(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
		ForEach.create();
		const forEachItemGenFunction = (_item, i: number) => {
			const v = _item;
			(parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
				MenuItem.create({
					content: v.itemName,
					symbolStartIcon: new SymbolGlyphModifier(v.startIcon).fontColor(v.startIconColor),
					symbolEndIcon: isShowCheck ? menuSelect === v.itemValue || menuSelect == i ? new SymbolGlyphModifier({ "id": 125831490, "type": 40000, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' }) : undefined : null
				});
				MenuItem.onChange((selected) => {
					if (menuList) {
						const onMenuClick = menuList.map((item) => item.onSelect)[i];
						if (onMenuClick) {
							onMenuClick();
						}
					}
					menuSelect = i;
					onMenuSelect(i);
				});
			}, MenuItem);
			MenuItem.pop();
		};
		this.forEachUpdateFunction(elmtId, menuList, forEachItemGenFunction, (menuItem: MenuItem) => JSON.stringify(menuItem), true, false);
	}, ForEach);
	ForEach.pop();
	Menu.pop();
}
class XbMenuPage extends ViewPU {
	constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
		super(parent, __localStorage, elmtId, extraInfo);
		if (typeof paramsLambda === 'function') {
			this.paramsGenerator_ = paramsLambda;
		}
		this.__selected = new ObservedPropertySimplePU(0, this, 'selected');
		this.setInitiallyProvidedValue(params);
		this.finalizeConstruction();
	}
	setInitiallyProvidedValue(params: XbMenuPage_Params) {
		if (params.selected !== undefined) {
			this.selected = params.selected;
		}
	}
	updateStateVars(params: XbMenuPage_Params) {}
	purgeVariableDependenciesOnElmtId(rmElmtId) {
		this.__selected.purgeDependencyOnElmtId(rmElmtId);
	}
	aboutToBeDeleted() {
		this.__selected.aboutToBeDeleted();
		SubscriberManager.Get().delete(this.id__());
		this.aboutToBeDeletedInternal();
	}
	private __selected: ObservedPropertySimplePU<number>;
	get selected() {
		return this.__selected.get();
	}
	set selected(newValue: number) {
		this.__selected.set(newValue);
	}
	initialRender() {
		this.observeComponentCreation2((elmtId, isInitialRender) => {
			Column.create();
		}, Column);
		Xb_Menu.bind(this)([{
			itemName: 'a',
			startIcon: { "id": 125831490, "type": 40000, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' },
			startIconColor: [Color.Black],
			itemValue: 0,
			onSelect: () => {}
		}, {
			itemName: 'b',
			startIcon: { "id": 125831490, "type": 40000, params: [], "bundleName": 'com.example.demo', "moduleName": 'entry' },
			startIconColor: [Color.White],
			itemValue: 1
		}], (index: number) => {
			this.selected = index;
		}, true, 0);
		Column.pop();
	}
	rerender() {
		this.updateDirtyElements();
	}
	static getEntryName(): string {
		return 'XbMenuPage';
	}
}
registerNamedRoute(() => new XbMenuPage(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
