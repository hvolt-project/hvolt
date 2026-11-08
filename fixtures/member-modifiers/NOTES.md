# `_pending-member-modifiers` — the source's access modifier is not carried, excluded from the default suite

Both protocols emit the access modifier the source wrote; hvolt drops it and substitutes its own
default (measured 2026-11-07 with `HOME=<writable dir> scripts/oracle/run-hvigor.sh _pending-member-modifiers`).

| member | reference | hvolt |
|---|---|---|
| `private hidden: number` (V2) | `private hidden: number;` | `hidden: number;` |
| `public shown: number` (V2) | `public shown: number;` | `shown: number;` |
| `public visible: number` (V1) | `public visible: number;` | `private visible: number;` |

Source of the rule:

* V1 — `createPropertyDeclaration` (`process_component_class.js`) sets the modifiers to
  `concatenateDecoratorsAndModifiers(undefined, s || [private])`, where `s` is
  `getModifiers(node)`: the source's own modifiers when it wrote any, `private` otherwise.
* V2 — `processComponentProperty` (`process_struct_componentV2.js`) ends with
  `updatePropertyDeclaration(e, concatenateDecoratorsAndModifiers(s, getModifiers(e)), …)`: the
  declaration keeps exactly the modifiers written, and none are added.

`FieldInfo` (`src/build/stage/utils.rs`) records `is_readonly` and `is_static` but not the access
modifier, so neither stage can reproduce it. The fix is one more fact at parse time
(`lower/ets_component.rs` reads `prop.accessibility` / `prop.public` off the oxc node) plus its
use in `emit_v1_field_declaration` and `emit_v2_field_declaration`.

Run the fixture explicitly (`scripts/oracle/all.sh _pending-member-modifiers`) to see the
failure; rename the directory once it passes.
