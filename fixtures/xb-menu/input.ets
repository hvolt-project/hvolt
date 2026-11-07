import { MenuItem } from "../settings/SettingsCommonOp"
import { SymbolGlyphModifier } from "@kit.ArkUI"

@Builder
export function Xb_Menu(
  menuList: MenuItem[],
  onMenuSelect: (index: number) => void,
  isShowCheck: boolean,
  menuSelect?: string | number,
) {
  Menu() {
    ForEach(menuList, (v: MenuItem, i: number) => {
      MenuItem({
        content: v.itemName,
        symbolStartIcon: new SymbolGlyphModifier(v.startIcon).fontColor(v.startIconColor),
        symbolEndIcon: isShowCheck ?
          (menuSelect === v.itemValue || menuSelect == i  ? new SymbolGlyphModifier($r('sys.symbol.checkmark')) : undefined) :
          null
      })
        .onChange((selected) => {
          if (menuList) {
            const onMenuClick = menuList.map(item => item.onSelect)[i]
            if (onMenuClick) {
              onMenuClick()
            }
          }
          menuSelect = i
          onMenuSelect(i)
        })
    }, (menuItem: MenuItem) => JSON.stringify(menuItem))
  }
}
@Entry
@Component
struct XbMenuPage {
  @State selected: number = 0

  build() {
    Column() {
      Xb_Menu(
        [
          { itemName: 'a', startIcon: $r('sys.symbol.checkmark'), startIconColor: [Color.Black], itemValue: 0, onSelect: () => {} },
          { itemName: 'b', startIcon: $r('sys.symbol.checkmark'), startIconColor: [Color.White], itemValue: 1 }
        ],
        (index: number) => {
          this.selected = index
        },
        true,
        0
      )
    }
  }
}
