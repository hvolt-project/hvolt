@Entry
@Component
struct SharedPage {
  build() {
    Text('shared')
  }
}

@Builder
export function pageBuilder(text: string) {
  Text(text)
}
