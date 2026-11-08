# configuration-page

The first real-file fixture that is a **page** rather than a component:
`entry/pages/ConfigurationPage.ets` from clashbox (1029 lines), `@Entry` and all. The two
earlier real-file fixtures (`proxy-node-item`, `proxy-group-item`) are components, so the entry
point emission — `registerNamedRoute`, the `@Entry` struct itself — had no real-file coverage
before this one.

Source: `~/DshHome/clashbox-tailscale/clashbox/entry/pages/ConfigurationPage.ets`
(also at `clashbox/entry/src/main/ets/pages/ConfigurationPage.ets`, same content).

## How `input.ets` was derived

`input.ets` = stub header (348 lines) + **the original source verbatim from line 47** (983 lines).
Only the original import block (lines 1-46) was replaced; everything else is byte-identical:

```sh
sed -n '349,$p' fixtures/configuration-page/input.ets | cmp - \
  <(sed -n '47,$p' ~/DshHome/clashbox-tailscale/clashbox/entry/pages/ConfigurationPage.ets)
```

The `@kit.*` / `@ohos.*` imports are kept (ArkUI, PerformanceAnalysisKit, base, ShareKit, ArkData,
AbilityKit, BasicServicesKit, CoreFileKit, ArkTS): the SDK provides them, and their rewrite is part
of what the fixture pins.

## Stubs (imports folded into the file)

`Constants` (11 constants incl. `NewConfigData`), `ConfigData`/`SubscriptionInfo`,
`ProfileType`, `AppConfig`/`UIConfig`, `EventHub`/`EventKey`, `getResourceString`,
`convertTime`, `ListGutter`/`ListLanes`/`NavigationListPadding`/`TabContentTitleMargin`,
`isON`/`customAnimationUtil`, `customVibrator`, `ClashViewModel`/`SocketProxy`, `ShareData`,
`Xb_GetResourceString`/`Xb_TabBarVertical`/`Xb_Share`/`Xb_PreferenceUtil`/`Xb_ToastUtil`,
`PopTips`/`PopupBuilder`, and the components `CellularData`, `Nothing`, `TopBar`, `RoundButton`,
`ConfigAdd`, `ImportConfigFromURL`, `ImportConfigFromFile`, `QRCodeDialog`, `CustomConfirmDialog`.

One stub detail that is not obvious: the `@CustomDialog` stub **must carry a
`controller?: CustomDialogController` member**, or the reference refuses to compile
(`The '@CustomDialog' decorated custom component must contain a property of the
CustomDialogController type`) and no golden is emitted.

## Resources the fixture needs

The page references resource names the template project does not declare, and the ArkTS compiler
fails on them (`10903329 ArkTS Compiler Error: Unknown resource name`). Added to
`project/resources/base/element/*.json`:

```
color:  background, container_background, float_tab_background_blur,
        list_item_background_color, shadow_button
string: Cancel, Collect, Collect_Tip, Configuration, Edit, click_error_prompt,
        configuration_deleted, configuration_shared, create_copy, delete,
        delete_current_configuration, export_config_file, export_config_file_success,
        failed_load_config, favorite_config_no, generate_QR_code, qrcode_share, share, sort,
        uncollected, uncollected_tip, update, update_by, updated_success_config, updating_config
media:  shared.png  (`$r('app.media.shared')`; copied from the template's startIcon.png)
```

**`new-fixture.sh` re-rsyncs the template and overwrites all of these**, so re-running it puts the
fixture back into a state where hvigor refuses to build. Re-apply the three lists afterwards.

## State

`scripts/oracle/all.sh configuration-page` reports **only** `comment text differs (521 vs 356)` —
**no `AST differs`**. The emitted tree is identical to the reference's; the whole difference is
comment accounting (16 comments dropped by hvolt, 32 emitted fewer times than the reference). See
the report in the task that added the fixture.