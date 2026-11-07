# `_pending-v2-reuse` — the `@ReusableV2` call site is not implemented

Two of the three emissions a `@ReusableV2` struct needs are in place; the call site is the
remaining one (measured 2026-11-07 with
`HOME=<writable dir> scripts/oracle/run-hvigor.sh _pending-v2-reuse`).

Done, and pinned separately by `fixtures/v2-reusable-class`:

1. the file-level helper
   `function __ReusableV2_Inner_Decorator__(BaseClass) { Reflect.defineProperty(BaseClass.prototype, "isReusable_", { get: () => true }); }`,
   emitted once per file right after the `checkFinalizeConstruction` / `checkContextStack`
   headers;
2. the class decorator `@__ReusableV2_Inner_Decorator__`, ahead of `export`.

Still open — the parent's call site. The reference replaces the whole
`observeComponentCreation2` / `ViewV2.create` frame with one call
(`generateReuseOrCreateArgArr` + `createCustomComponent(…, p=REUSE_OR_CREATE_METHOD)` in
`process_custom_component.js`):

```ts
this.reuseOrCreateNewComponent({
    componentClass: ReuseKid,
    getParams: () => ({
        title: `${this.n}`
    }),
    getReuseId: getUINativeModule().common.getApiTargetVersion() >= 26 ? () => "ReuseKid" : () => "ReuseKid",
    extraInfo: { page: "entry/src/main/ets/pages/Index.ets", line: 8, col: 7 }
});
```

* `componentClass` is the child's expression as written.
* `getParams` is an arrow returning the call site's params object verbatim (parenthesised),
  i.e. the same object the constructor would take, with no `@Link` rewriting (V2 has none).
* `getReuseId` is `createConditionalExpression(apiVersionCheck(26), f.reuseAttribute ?? () => <name>, f.reuse.length ? f.reuse : () => <name>)`;
  without attribute-level reuse ids both branches are the child's name, as above.
* `extraInfo` is the object literal the frame would pass as the construction's sixth argument.

Source: `createCustomComponent` selects the method name (`REUSE_OR_CREATE_METHOD` when the
`isReuseInV2` flag is set) and passes the single object from `generateReuseOrCreateArgArr`.

Run the fixture explicitly (`scripts/oracle/all.sh _pending-v2-reuse`) to see the failure;
rename the directory once the call site lands.