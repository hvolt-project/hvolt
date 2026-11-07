interface Pair {
  name: string
  key: string
  value: number
}

interface Nested {
  inner: Pair
}

function getFn(): string {
  return 'from-fn'
}

@Component
struct ChildRow {
  @Prop name: string = ''
  @Prop tag: string = ''

  build() {
    Row() {
      Text(this.name)
      Text(this.tag)
    }
  }
}

@Component
@Reusable
struct ReusableRow {
  @Prop name: string = ''

  build() {
    Row() {
      Text(this.name)
    }
  }
}

@Entry
@Component
struct ShorthandProperty {
  @State name: string = 'n'
  @State ident: string = 'i'
  @State items: Pair[] = []

  build() {
    Column() {
      Text(this.describe())
      ChildRow({ name: this.name, tag: this.ident })
      ReusableRow({ name: this.name })
    }
  }

  describe(): string {
    const name = this.name
    const key = 'k'
    const value = 1
    // 1. 键与值同名
    const same: Pair = { name: name, key: key, value: value }
    // 2. 键与值不同名
    const other: Pair = { name: key, key: name, value: value }
    // 3. 值是成员访问
    const member: Pair = { name: this.name, key: this.ident, value: value }
    // 4. 值是调用
    const called: Pair = { name: getFn(), key: getFn(), value: value }
    // 5. 嵌套对象字面量里的简写
    const nested: Nested = { inner: { name: name, key: key, value: value } }
    // 6. 箭头函数返回对象里的简写
    const make = (): Pair => ({ name: name, key: key, value: value })
    // 7. 数组元素是对象字面量时的简写
    const list: Pair[] = [{ name: name, key: key, value: value }]
    this.items = list
    return `${same.name}${other.name}${member.name}${called.name}${nested.inner.name}${make().name}`
  }
}
