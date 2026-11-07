// The V2 counterpart of `static-member-modifier`: which modifiers a `static` member keeps in a
// `@ComponentV2` struct's emitted class, next to a `@Local` member as the decorated control.
// Each field is read once in `build()` so nothing can be shaken away. (`static name` is not
// usable here: TypeScript rejects it as a conflict with `Function.name`.)
@Entry
@ComponentV2
struct V2StaticMemberModifier {
  public static count: number = 0;
  private static label: string = 'a';
  static plain: boolean = false;
  @Local public value: number = 1;

  build() {
    Column() {
      Text(`${V2StaticMemberModifier.count}`)
      Text(V2StaticMemberModifier.label)
      Text(`${V2StaticMemberModifier.plain}`)
      Text(`${this.value}`)
    }
  }
}
