// 装饰成员与普通字段上方的注释是否保留。`compare.js` 把注释单独比对（格式不算差异，
// 注释算），所以这条一旦丢，凡是字段带注释的文件都会报差异。
@Component
struct V1CommentProbe {
  // V1 状态字段前面的注释
  @State count: number = 0;
  // 普通字段前面的注释
  private label: string = 'probe';

  build() {
    Column() {
      Text(this.label)
    }
  }
}

@ComponentV2
struct V2CommentProbe {
  // V2 @Local 前面的注释
  @Local n: number = 0;
  // V2 @Param 前面的注释
  @Param title: string = '';

  build() {
    Column() {
      Text(`${this.n}`)
    }
  }
}

@Entry
@Component
struct MemberLeadingComment {
  build() {
    Column() {
      V1CommentProbe()
      V2CommentProbe()
    }
  }
}