namespace Utils {
  export function helper(): number {
    return 1;
  }
}

export = Utils;

@Entry
@Component
struct ExportAssignProbe {
  build() {
    Column() {
      Text(`${Utils.helper()}`)
    }
  }
}
