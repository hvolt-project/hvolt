# readonly-member

Which accessibility keyword a `readonly` struct member keeps in the reference's emitted class,
and whether the default `private` still applies to it.

The non-static path of `createPropertyDeclaration` (`process_component_class.js`) declares a
member without an accessibility modifier as `private`. A `readonly` member is the exception: the
reference keeps the source's modifier list and adds nothing, so the declaration comes out
`readonly <name>` — no `private`. The decorated path follows the same rule on the backstore:
`@State readonly st` becomes `readonly __st: ObservedPropertySimplePU<number>;`.

Measured with `HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh readonly-member`
(`golden/Index.ts`, member declarations):

| source | emitted |
| --- | --- |
| `readonly ro: number = 2` | `readonly ro: number;` |
| `private readonly pr: number = 3` | `private readonly pr: number;` |
| `public readonly pu: number = 4` | `public readonly pu: number;` |
| `@State readonly st: number = 5` | `readonly __st: ObservedPropertySimplePU<number>;` |
| `plain: string = 'a'` (control) | `private plain: string;` |
| `private plainPrivate: string = 'b'` (control) | `private plainPrivate: string;` |

The initializer never stays on a non-static declaration: it moves into the constructor
(`this.ro = 2;`), `readonly` or not. `static readonly TAG` is covered by `static-member` and
`v2-static-member`; V2 adds no `private` default at all (`process_struct_componentV2.rs` emits
`{prefix}{readonly}{name}`), so only the V1 path needed the fix.

Before the fix `process_component_member.rs`'s plain-field branch defaulted to `private`
unconditionally (`f.accessibility.as_deref().unwrap_or("private")`), which made `readonly ro`
come out `private readonly ro` and turned this fixture red.
