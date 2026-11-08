export interface BindSheetTitleCustomBuilderMenu {
  symbol?: Resource
  symbolFontColor?: Array<ResourceColor>
  symbolBackgroundColor?: ResourceColor
  isEnabled?: boolean
  action?: Function
}

export interface PaddedArea {
  left?: Length
  right?: Length
  top?: Length
  bottom?: Length
}

export interface BindSheetTitleCustomBuilderParam {
  pageInfos: NavPathStack
  isSubPage: boolean
  titlePaddingLeft?: Length
  mainTitle: ResourceStr
  subTitle?: ResourceStr
  mainTitleFontWeight?: number
  mainTitleFontSize: number
  subTitleFontSize?: number
  titleBackgroundColor?: Length
  padding?: PaddedArea
  menus: Array<BindSheetTitleCustomBuilderMenu>
  back?: () => void
  needSetHeight?: boolean
}

@Builder
export function BindSheetTitleCustomBuilder(param: BindSheetTitleCustomBuilderParam) {
  Row() {
    Text(param.mainTitle)
  }
}
