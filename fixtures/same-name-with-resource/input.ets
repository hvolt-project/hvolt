interface Info {
  label: string
  icon: Resource
}

@Entry
@Component
struct SameNameWithResource {
  describe(): string {
    const label = 'x'
    const info: Info = { label: label, icon: $r('app.string.app_name') }
    return info.label
  }

  build() {
    Column() {
      Text(`${this.describe()}`)
    }
  }
}
