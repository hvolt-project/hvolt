export interface MenuItem {
  selectOption?: SelectOption
  startIcon?: Resource
  startMediaIcon?: Resource
  startIconColor?: ResourceColor[]
  endIcon?: Resource
  itemName?: ResourceStr
  itemValue?: string | number
  onSelect?: () => void
}
