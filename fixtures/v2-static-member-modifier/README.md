# v2-static-member-modifier

The V2 counterpart of `static-member-modifier`: which modifiers a `static` member keeps in a
`@ComponentV2` struct's emitted class, with a `@Local` member as the decorated control.

The reference lowers a static member through `staticPropertySet` (`process_component_property`
hands the initializer back to the declaration, then `updatePropertyDeclaration` reuses
`getModifiers(e)`), so the declaration is emitted **as written** — and that includes the
accessibility modifier. No default is added.

Measured with `HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh v2-static-member-modifier`
(`golden/Index.ts:13-16`):

```ts
    public static count: number = 0;
    private static label: string = 'a';
    static plain: boolean = false;
    @Local
    public value: number;
```

**V2 follows the same rule as V1 here** — the modifier is kept, none is invented, and the order is
`[access] static [readonly] name[?]`. There is no V1/V2 divergence for static members. (The V1
measurement is `fixtures/static-member-modifier/golden/Index.ts:30-33`.)

Before the fix `process_struct_componentV2.rs`'s static branch hardcoded `"    static …"` and never
read `f.accessibility`, so the first two lines came out `static count` / `static label` and this
fixture was red. `fixtures/v2-static-member` only writes `static readonly TAG`, which has no
modifier to lose — that is why the gap survived it.

The `@Local` control is the non-static path: it keeps `public` and drops the initializer into the
constructor. The reference puts `@Local` on its own line above the declaration; hvolt writes it on
one line (`@Local public value: number;`). That is a formatting difference only — the AST
comparison treats both the same.

## Two things that do not work here

- `static name` — TypeScript rejects it: *Static property 'name' conflicts with built-in property
  'Function.name' of constructor function*. The fixture uses `label` instead.
- More than one static member is fine; a single static member alone is fine. Earlier probe
  variants confirmed both compile, so the only blocker was the `name` collision.

Each field is read once in `build()` so nothing can be shaken out of the emitted class.