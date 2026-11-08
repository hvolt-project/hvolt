// 顶部注释：这是中文说明，含全角标点“引号”与（括号）
/* 块注释：中文里的花括号 { 与反引号 ` 都是数据 */
@Component
struct UnicodeComments {
  @State 计数: number = 0

  // 方法前的注释：用于验证注释不会被改写
  build() {
    Column() {
      // 行注释里的分号; 与逗号, 与冒号: 都不应影响结构
      Text(`当前值：${this.计数}，单位“个”`)
        .fontSize(16)
      Button('点一下加一')
        .onClick(() => {
          // 自增
          this.计数 += 1
        })
    }
    .width('100%')
  }
}

@Entry
@Component
struct Index {
  @State 标题: string = '中文标题'

  build() {
    Column({ space: 8 }) {
      Text(this.标题).fontSize(20)
      UnicodeComments()
    }
    .padding(12)
  }
}
