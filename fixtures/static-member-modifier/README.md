# static-member-modifier

Which modifiers a `static` struct member keeps in the reference's emitted class, and whether a
static member written without one gains a default.

The reference lowers a static member through `isStaticProperty`
(`process_component_class.js`): the declaration is pushed through **as written** — no backstore,
no accessors, the initializer stays on the declaration — and that "as written" includes the
accessibility modifier. It adds no default, unlike the non-static path, which defaults to
`private` (`createPropertyDeclaration`'s `s || [private]`).

Measured with `HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh static-member-modifier`
(`golden/Index.ts:30-33`):

```ts
public static currentTouchProxyData: string = 'a';
private static count: number = 0;
static plain: boolean = false;
public name: string;
```

So all four source forms come out word for word: `public static` keeps its modifier, `private
static` keeps its modifier, a bare `static` stays bare, and the non-static `public name` (the
control) keeps `public` while its initializer moves into the constructor.

Before the fix `src/build/stage/process_component_member.rs`'s static branch hardcoded
`"    static {}{name}…"` and never read `f.accessibility`, so the first two lines came out as
`static currentTouchProxyData` / `static count` and this fixture was red. The existing
`static-member` and `v2-static-member` fixtures only ever wrote `static readonly TAG`, which has no
modifier to lose — that is why the gap survived them.

Each field is read once in `build()` so nothing can be shaken out of the emitted class.