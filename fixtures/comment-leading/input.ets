// Where the reference puts a comment written at each of five positions. Every marker is unique so
// `grep -c` in `golden/Index.ts` decides whether the position survives and how many copies it gets.
// The expected placements are recorded in README.md; this file is the acceptance fixture for the
// comment-emission path, so a fix is judged by those counts rather than by eyeballing a diff.

class ProbeDataSource implements IDataSource {
  private listeners: DataChangeListener[] = []
  private data: string[] = []

  totalCount(): number {
    return this.data.length
  }

  getData(index: number): string {
    return this.data[index]
  }

  registerDataChangeListener(listener: DataChangeListener): void {
    this.listeners.push(listener)
  }

  unregisterDataChangeListener(listener: DataChangeListener): void {
    const pos = this.listeners.indexOf(listener)
    if (pos >= 0) {
      this.listeners.splice(pos, 1)
    }
  }
}

const INIT_VALUE: number = 9

const INIT_OBJ: ObjKind = { a: 3 }

interface ObjKind {
  a: number
}

enum LocalKind {
  A = 0,
  B = 1
}


@CustomDialog
struct ProbeDialog {
  controller?: CustomDialogController
  @Prop title: ResourceStr

  build() {
    Column() {}
  }
}

// ANIM_TOP
@AnimatableExtend(Text)
function animWidth(width: number) {
  .width(width)
}

// EXTEND_TOP
@Extend(Text)
function loudExtend() {
  .fontWeight(FontWeight.Bold)
}



@Component
struct ConsumeCarryProbe {
  @State first: number = 1
  // CARRY_ADJACENT
  @Consume('adjacent') adjacent: string

  helperMethod(): number {
    return 1
  }

  // CARRY_AFTER_METHOD
  @Consume('afterMethod') afterMethod: string

  build() {
    Column() {}
  }

  // CARRY_AFTER_BUILD
  @Consume('afterBuild') afterBuild: string
}

@Component
struct ConsumeFirstProbe {
  // CARRY_FIRST
  @Consume('firstField') firstField: string

  build() {
    Column() {}
  }
}

@Reusable
@Component
struct AttrChainProbe {
  @State wide: boolean = true

  build() {
    Column() {
      // ATTR_REUSABLE
      Text('r')
        .fontSize(12)
        .fontColor(Color.Red)
        .width(this.wide ? 10 : 20)
    }
  }
}

@Entry
@Component
struct CommentLeading {
  @State cond: boolean = true
  @State elseCond: boolean = false
  @State items: string[] = ['x']
  private source: ProbeDataSource = new ProbeDataSource()

  /* 自定义弹框 */
  // controller 定义
  private dlg: CustomDialogController = new CustomDialogController({
    builder: ProbeDialog({ title: 'probe' }),
    autoCancel: true
  })

  @State gapFirst: number = 17

  helperGap(): number {
    return 1
  }

  // GAP_AFTER_METHODS
  plainAfterGap: number = 19

  @State objFirst: ObjKind = { a: 1 }
  // AFTER_OBJ
  plainAfterObj: number = 7

  @State objIdentFirst: ObjKind = INIT_OBJ
  // BEFORE_OBJ_IDENT
  plainAfterObjIdent: number = 13

  @State identFirst: number = INIT_VALUE
  // BEFORE_IDENT
  plainAfterIdent: number = 11

  @State multiFirst: number = 5
  // BEFORE_MULTI_OBJ
  objMulti: ObjKind = {
    a: 2
  }

  @State numFirst: number = 3
  // BEFORE_OBJ
  objAfter: ObjKind = { a: 1 }

  /* MEMBER_PLAIN */
  dlg2: CustomDialogController = new CustomDialogController({
    builder: ProbeDialog({ title: 'plain' }),
    autoCancel: true
  })

  /* MEMBER_PUBLIC */
  public dlg3: CustomDialogController = new CustomDialogController({
    builder: ProbeDialog({ title: 'public' }),
    autoCancel: true
  })

  /* MEMBER_STATE */
  // MEMBER_STATE_SECOND
  @State tagged: number = 1


  build() {
    Column() {
      // IF_START
      if (this.cond) {
        Text('a')
      }
      if (this.elseCond) {
        Text('no')
      } else {
        // ELSE_FIRST
        Text('b')
      }
      // FOREACH_START
      ForEach(this.items, (item: string) => {
        Text(item)
      }, (item: string) => item)
      if (this.cond) {
        Text('c')
      }
      // AFTER_BLOCK
      Text('d')

      // BLOCK_ONE
      // BLOCK_TWO
      Text('x')

      /* BLOCK_A */
      // BLOCK_B
      Text('y')

      // TRIO_ONE
      // TRIO_TWO
      // TRIO_THREE
      Text('z')

      // BRANCH_IF
      if (this.cond) {
        Text('a')
      }
      // BRANCH_ELSEIF
      else if (this.elseCond) {
        Text('b')
      }
      // BRANCH_ELSE
      else {
        Text('c')
      }

      if (this.cond) {
        Text('d')
      }
      // BETWEEN_BRANCH
      else {
        Text('e')
      }

      // ANIM_CALL
      Text('an1')
        .fontSize(8)
        .animWidth(20)

      // EXTEND_CALL
      Text('e1')
        .fontSize(9)
        .loudExtend()

      // ATTR_ONE
      Text('a1')
        .fontSize(12)

      // ATTR_TWO
      Text('a2')
        .fontSize(12)
        .fontColor(Color.Red)

      // ATTR_FOUR
      Text('a4')
        .fontSize(12)
        .fontColor(Color.Red)
        .width(100)
        .height(20)

      // ATTR_COND
      Text('cond')
        .fontSize(this.cond ? 12 : 14)
        .width(100)

      // ATTR_RES
      Text('res')
        .fontColor($r('sys.color.font_primary'))
        .fontSize(12)

      AttrChainProbe()
      ConsumeCarryProbe()
      ConsumeFirstProbe()

      if (this.cond) {
        // CHILD_CALL
        AttrChainProbe()
      }

      List() {
        // LAZY_START
        LazyForEach(this.source, (item: string) => {
          ListItem() {
            Text(item)
          }
        }, (item: string) => item)
      }
      if (this.elseCond) {
        Text('tail')
      }
      // IF_TAIL
    }
  }
}
