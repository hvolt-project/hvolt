// Which modifiers a static struct member keeps in the reference's emitted class, and whether a
// static member with no modifier gains one. A `@Prop`-free `@Component` so the fields are plain
// declaration output; each field is read once in `build()` so nothing can be shaken away.
@Entry
@Component
struct StaticMemberModifier {
  public static currentTouchProxyData: string = 'a';
  private static count: number = 0;
  static plain: boolean = false;
  public name: string = 'b';

  build() {
    Column() {
      Text(StaticMemberModifier.currentTouchProxyData)
      Text(`${StaticMemberModifier.count}`)
      Text(`${StaticMemberModifier.plain}`)
      Text(this.name)
    }
  }
}
