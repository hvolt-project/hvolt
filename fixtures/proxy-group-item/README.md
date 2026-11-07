# proxy-group-item

A real component file from clashbox, replayed as a fixture: the whole
`xb_components/src/main/ets/components/Proxy/ProxyGroupItem.ets` page component, with every
project-local import turned into a same-file stub so it builds standalone.

Source: `~/DshHome/clashbox-tailscale/clashbox-flat/entry/components/Proxy/ProxyGroupItem.ets`
(494 lines). The same file exists at `clashbox/entry/src/main/ets/components/Proxy/ProxyGroupItem.ets`
under the DevEco layout and the two are byte-identical.

## How `input.ets` was derived

`input.ets` = stub header + **the original source verbatim from its line 19** + a host page.

Only the original import block (lines 1-18) was replaced; everything else is byte-identical, and
that is checkable:

```sh
sed -n '191,666p' fixtures/proxy-group-item/input.ets | cmp - \
  <(sed -n '19,$p' ~/DshHome/clashbox-tailscale/clashbox-flat/entry/components/Proxy/ProxyGroupItem.ets)
```

The `@kit.*` imports are kept on purpose — the SDK provides them, and their rewrite
(`@kit.PerformanceAnalysisKit` → `@ohos.hilog`, `$r()` → resource id) is part of what the
fixture is meant to pin.

## Stubs (imports that were folded into the file)

| original import | stub |
|---|---|
| `../../common/entity/Constants` | `const BORDER_RADIUS_20 / CONTENT_END_OFFSET_150 / LIST_ITEM_HEIGHT / TAB_CONTENT_TITLE_SYMBOL_FONT_SIZE` |
| `../../common/datasources/ProxyData` | `class ProxyItem`, `class ProxyItemDataSource implements IDataSource`, `class ProxyGroupItemDataSource` |
| `proxy_core` | `interface Profile`, `enum ProxySort` |
| `../../entryability/AppState` | `enum ProxyCardSize / ProxyGroupType / ClashCore`, `class AppConfig`, `class UIConfig`, `class AppState` |
| `../../common/utils/Animation` | `class customAnimationUtil` (two statics) |
| `../../common/utils/VibratorUtil` | `class customVibrator` |
| `../../entryability/ClashViewModel` | `class ClashViewModel` (default import → class) |
| `../Start/Popup` | `const PopTips`, `@Builder function PopupBuilder(ctx, tip, onClose)` |
| `../../common/breakpoint/BreakPoint` | `function ListGutter(): Dimension`, `function ProxyListLanes(): number` |
| `xb_components` | `class Xb_ToastUtil`, `interface ToastOptions` |
| `./ProxyNodeItem` | `@Component struct ProxyNode` with the props the call site passes |

Two more things in the fixture are not in the original file:

- `@Entry @Component struct ProxyGroupItemPage` appended at the end. The route page needs an
  `@Entry` component or the reference build emits no entry point. It is an empty `Column()`,
  the same shape `proxy-node-item` uses.
- `ListGutter` returns `Dimension`, not `Length`: `lanes()` takes `Dimension` and a `Length`
  return type fails the ArkTS compile, which stops the reference before it emits anything.

## Resource declarations the fixture needs

The component references `$r('app.string.*')` / `$r('app.color.*')` names that the template
project does not declare, and the ArkTS compiler fails on them
(`10903329 ArkTS Compiler Error: Unknown resource name 'Speed_Delay'`). They were added to
`project/resources/base/element/string.json` and `color.json`:

```
string: Test_Delay, Collect_Tip, favorite_proxy_no, uncollected_tip, Speed_Delay,
        Speed_Delay_Content, Sure
color:  container_background, divider
```

**`new-fixture.sh` re-rsyncs the template and overwrites both files**, so re-running it puts the
fixture back into a state where hvigor refuses to build. Re-apply the two lists afterwards.

## State

`scripts/oracle/all.sh proxy-group-item` fails, on three translation differences plus the
comment accounting. See the report in the task that added the fixture; `README` only records how
the fixture was built.