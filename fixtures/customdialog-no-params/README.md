# customdialog-no-params

A `@CustomDialog` invoked with **no parameters** from inside a `CustomDialogController`. The
reference synthesises the construction itself, and with no parameters the object slot is an empty
`{}` — not an empty string.

`golden/Index.ts:59`:

```js
let jsDialog = new ProbeDialog(this, {}, undefined, -1, () => { }, { page: "entry/src/main/ets/pages/Index.ets", line: 23, col: 14 });
jsDialog.setController(this.dlg);
ViewPU.create(jsDialog);
let paramsLambda = () => {
    return {};
};
```

Before the fix hvolt emitted the slot as nothing:

```js
let jsDialog = new ProbeDialog(this, , undefined, -1, () => { }, { page: …, line: 23, col: 14 });
```

which is not valid TypeScript — `es2abc` then rejected the *whole* file
(`✗ es2abc merge failed: SyntaxError: Primary expression expected [Index.ts:57:45]`), so
`all.sh customdialog-no-params` reported `ACTUAL FAIL` and produced no `actual.ts` at all. The
`paramsLambda` body was affected the same way (`return ;` instead of `return {};`).

The parameter object is only missing for this path: a plain component called with no arguments
(`ConfigAdd()`) is emitted as `new ConfigAdd(this, {}, undefined, elmtId, …)` on both sides.

## Fix

`src/build/ets_assemble.rs`, the `params` a `builder:` site hands to
`custom_dialog_builder` (`rewrite_dialog_builders`): an argument list that trims to empty becomes
`{}`. That one binding is spliced into both the construction and the `paramsLambda` body, so the
two symptoms had one cause.

## Notes

- `@CustomDialog` needs a `controller?: CustomDialogController` member or the reference refuses to
  compile the file at all.
- `comment-leading` deliberately does **not** cover this shape: it passes `{ title: 'probe' }` so
  that the comment question stays separable from this one.