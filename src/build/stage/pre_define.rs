// process stage: `pre_define.js` — every literal the ETS transform emits, under the
// reference's own symbol name.
//
// A rule reads a name, not a string. Each constant below is copied from
// `ets-loader/lib/pre_define.js`; the right-hand comment is the reference's export, so a
// spelling question is answered by grepping that file instead of reading an emitted file back.
#![allow(dead_code)] // the reference's vocabulary: each stage reads the names it needs

// ── decorators ───────────────────────────────────────────────────────────────────────────
pub(crate) const COMPONENT_DECORATOR_ENTRY: &str = "@Entry"; // COMPONENT_DECORATOR_ENTRY
pub(crate) const COMPONENT_DECORATOR_PREVIEW: &str = "@Preview"; // COMPONENT_DECORATOR_PREVIEW
pub(crate) const COMPONENT_DECORATOR_COMPONENT: &str = "@Component"; // COMPONENT_DECORATOR_COMPONENT
pub(crate) const COMPONENT_DECORATOR_COMPONENT_V2: &str = "@ComponentV2";
pub(crate) const COMPONENT_DECORATOR_CUSTOM_DIALOG: &str = "@CustomDialog";
pub(crate) const COMPONENT_DECORATOR_REUSEABLE: &str = "@Reusable";
pub(crate) const COMPONENT_DECORATOR_REUSABLE_V2: &str = "@ReusableV2";
pub(crate) const COMPONENT_DECORATOR_NAME_COMPONENT: &str = "Component";
pub(crate) const COMPONENT_DECORATOR_NAME_CUSTOMDIALOG: &str = "CustomDialog";
pub(crate) const DECORATOR_REUSEABLE: &str = "Reusable";
pub(crate) const DECORATOR_REUSABLE_V2: &str = "ReusableV2";
pub(crate) const COMPONENT_STATE_DECORATOR: &str = "@State";
pub(crate) const COMPONENT_PROP_DECORATOR: &str = "@Prop";
pub(crate) const COMPONENT_LINK_DECORATOR: &str = "@Link";
pub(crate) const COMPONENT_STORAGE_PROP_DECORATOR: &str = "@StorageProp";
pub(crate) const COMPONENT_STORAGE_LINK_DECORATOR: &str = "@StorageLink";
pub(crate) const COMPONENT_PROVIDE_DECORATOR: &str = "@Provide";
pub(crate) const COMPONENT_CONSUME_DECORATOR: &str = "@Consume";
pub(crate) const COMPONENT_OBJECT_LINK_DECORATOR: &str = "@ObjectLink";
pub(crate) const COMPONENT_WATCH_DECORATOR: &str = "@Watch";
pub(crate) const COMPONENT_BUILDERPARAM_DECORATOR: &str = "@BuilderParam";
pub(crate) const COMPONENT_LOCAL_STORAGE_LINK_DECORATOR: &str = "@LocalStorageLink";
pub(crate) const COMPONENT_LOCAL_STORAGE_PROP_DECORATOR: &str = "@LocalStorageProp";
pub(crate) const COMPONENT_REQUIRE_DECORATOR: &str = "@Require";
pub(crate) const COMPONENT_ENV_DECORATOR: &str = "@Env";
pub(crate) const COMPONENTV2_LOCAL_DECORATOR: &str = "@Local";
pub(crate) const COMPONENTV2_PARAM_DECORATOR: &str = "@Param";
pub(crate) const COMPONENTV2_ONCE_DECORATOR: &str = "@Once";
pub(crate) const COMPONENTV2_EVENT_DECORATOR: &str = "@Event";
pub(crate) const COMPONENTV2_CONSUMER_DECORATOR: &str = "@Consumer";
pub(crate) const COMPONENTV2_PROVIDER_DECORATOR: &str = "@Provider";
pub(crate) const COMPONENT_BUILDER_DECORATOR: &str = "@Builder";
pub(crate) const COMPONENT_LOCAL_BUILDER_DECORATOR: &str = "@LocalBuilder";
pub(crate) const COMPONENT_EXTEND_DECORATOR: &str = "@Extend";
pub(crate) const COMPONENT_STYLES_DECORATOR: &str = "@Styles";
pub(crate) const COMPONENT_ANIMATABLE_EXTEND_DECORATOR: &str = "@AnimatableExtend";
pub(crate) const COMPONENT_OBSERVED_DECORATOR: &str = "@Observed";
pub(crate) const COMPONENT_OBSERVEDV2_DECORATOR: &str = "@ObservedV2";
pub(crate) const COMPONENT_SENDABLE_DECORATOR: &str = "@Sendable";
pub(crate) const COMPONENT_CONCURRENT_DECORATOR: &str = "@Concurrent"; // COMPONENT_CONCURRENT_DECORATOR

/// The directive `processConcurrent` prepends to the body of a `@Concurrent` function
/// declaration. The reference writes `createStringLiteral("use concurrent")` inline, so this
/// literal has no exported symbol of its own.
pub(crate) const CONCURRENT_DIRECTIVE: &str = "use concurrent";
pub(crate) const CLASS_TRACK_DECORATOR: &str = "Track";
pub(crate) const CLASS_MIN_TRACK_DECORATOR: &str = "Trace";

// ── component creation ───────────────────────────────────────────────────────────────────
pub(crate) const COMPONENT_CREATE_FUNCTION: &str = "create"; // COMPONENT_CREATE_FUNCTION
pub(crate) const COMPONENT_CREATE_CHILD_FUNCTION: &str = "createWithChild";
pub(crate) const COMPONENT_CREATE_LABEL_FUNCTION: &str = "createWithLabel";
pub(crate) const COMPONENT_CREATE_RECYCLE: &str = "createRecycle";
pub(crate) const COMPONENT_POP_FUNCTION: &str = "pop";
pub(crate) const COMPONENT_BUTTON: &str = "Button";
/// `TabContentAndNavDestination` (pre_define.js): the components `processTabAndNav` handles,
/// where the child block becomes the argument of `create` — `X.create(() => { …children… })` —
/// and the attribute chain is written after it, in source order.
pub(crate) const TAB_CONTENT_AND_NAV_DESTINATION: &[&str] = &["TabContent", "NavDestination"];

/// `NavDestination` (`pre_define.js`: `NAV_DESTINATION`). Only this one of the two takes the
/// page coordinate its `create` call carries (`navigationOrNavDestinationCreateContent`); a
/// `Navigation` takes a different second argument (`isUserCreateStack`) and a `TabContent` none.
pub(crate) const COMPONENT_NAV_DESTINATION: &str = "NavDestination";

/// `CUSTOM_BUILDER_PROPERTIES` (`component_map.js`): the attribute names whose argument is a
/// builder reference — `{ builder: … }` — rather than a value. Anything else (a
/// `backgroundColor`, a `width`, …) keeps the call as written.
pub(crate) const CUSTOM_BUILDER_PROPERTIES: &[&str] = &[
    "background",
    "bindPopup",
    "bindMenu",
    "bindContextMenu",
    "bindContextMenuWithResponse",
    "title",
    "menus",
    "toolBar",
    "tabBar",
    "toolbar",
    "onDragStart",
    "onItemDragStart",
    "swipeAction",
    "bindContentCover",
    "bindSheet",
    "navDestination",
    "overlay",
    "toolbarConfiguration",
    "customKeyboard",
    "bindSelectionMenu",
    // The tail of the reference table (`component_map.js`), read from the source rather than
    // from a missing-attribute diff: `description`, `showUnit`, `create`, `addBuilderSpan`,
    // `dragPreview`, `accessibilityVirtualNode`. The first two are also members of
    // `CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY`: their argument is a bare arrow function, not a
    // `{ builder: … }` object.
    "description",
    "showUnit",
    "create",
    "addBuilderSpan",
    "dragPreview",
    "accessibilityVirtualNode",
];

/// `CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY` (`component_map.js`): the builder properties whose
/// argument is emitted as a bare arrow (`() => { this.X.call(this) }`) instead of being wrapped
/// with a `builder:` key.
pub(crate) const CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY: &[&str] = &["showUnit", "create"];

/// `PROPERTIES_ADD_DOUBLE_DOLLAR` (`pre_define.js`): the component/property pairs whose `$$`
/// value is a two-way binding. A `$$` under any other property belongs to a style marker
/// (`bindSheet`, `visibility`, …) and must be left as written.
pub(crate) const DOUBLE_DOLLAR_PROPERTIES: &[(&str, &[&str])] = &[
    ("Radio", &["checked"]),
    ("TextTimer", &["format", "count", "isCountDown"]),
    ("Refresh", &["refreshing"]),
    ("Checkbox", &["select"]),
    ("CheckboxGroup", &["selectAll"]),
    ("DatePicker", &["selected"]),
    ("TimePicker", &["selected"]),
    ("MenuItem", &["selected"]),
    ("Panel", &["mode"]),
    ("Rating", &["rating"]),
    ("Search", &["value"]),
    ("SideBarContainer", &["showSideBar"]),
    ("Slider", &["value"]),
    ("Stepper", &["index"]),
    ("Swiper", &["index"]),
    ("Tabs", &["index"]),
    ("TextArea", &["text"]),
    ("TextInput", &["text"]),
    ("TextPicker", &["value", "selected"]),
    ("Toggle", &["isOn"]),
    ("AlphabetIndexer", &["selected"]),
    ("Select", &["selected", "value"]),
    ("Calendar", &["date"]),
    ("GridItem", &["selected"]),
    ("ListItem", &["selected"]),
];

/// `PROPERTIES_ADD_DOUBLE_EXCLAMATION` (`pre_define.js`): the component/property pairs whose
/// postfix `!!` value is a two-way binding.
///
/// The pair list is its own, not a copy of [`DOUBLE_DOLLAR_PROPERTIES`]: `Navigation.navBarWidth`,
/// `Scroll.zoomScale`, `ArcAlphabetIndexer.selected`, `SideBarContainer.sideBarWidth` and the
/// whole `ContainerReader` family appear only here. The emitted key differs too — `$$` writes
/// `changeEvent`, `!!` writes `$value` (`generateObjectForDollar` vs `generateObjectForExclamation`).
pub(crate) const EXCLAMATION_PROPERTIES: &[(&str, &[&str])] = &[
    ("Radio", &["checked"]),
    ("TextTimer", &["format", "count", "isCountDown"]),
    ("Refresh", &["refreshing"]),
    ("Checkbox", &["select"]),
    ("CheckboxGroup", &["selectAll"]),
    ("DatePicker", &["selected"]),
    ("TimePicker", &["selected"]),
    ("MenuItem", &["selected"]),
    ("Navigation", &["navBarWidth"]),
    ("Panel", &["mode"]),
    ("Rating", &["rating"]),
    ("Search", &["value"]),
    ("SideBarContainer", &["showSideBar", "sideBarWidth"]),
    ("Slider", &["value"]),
    ("Stepper", &["index"]),
    ("Swiper", &["index"]),
    ("Tabs", &["index"]),
    ("TextArea", &["text"]),
    ("TextInput", &["text"]),
    ("TextPicker", &["value", "selected"]),
    ("Toggle", &["isOn"]),
    ("AlphabetIndexer", &["selected"]),
    ("ArcAlphabetIndexer", &["selected"]),
    ("Select", &["selected", "value"]),
    ("Calendar", &["date"]),
    ("GridItem", &["selected"]),
    ("ListItem", &["selected"]),
    ("Scroll", &["zoomScale"]),
    (
        "ContainerReader",
        &["size", "widthBreakpoint", "heightBreakpoint"],
    ),
];

/// `STYLE_ADD_DOUBLE_EXCLAMATION` (`pre_define.js`): the attribute names whose first argument,
/// when it is written `expr!!`, becomes the `{ value, $value }` pair. The set has five members
/// and does **not** include `bindContextMenuWithResponse`, unlike `CUSTOM_BUILDER_PROPERTIES`.
pub(crate) const EXCLAMATION_ATTRIBUTES: &[&str] = &[
    "bindMenu",
    "bindContextMenu",
    "bindPopup",
    "bindContentCover",
    "bindSheet",
];

/// The keys `generateObjectForExclamation` writes: `$_VALUE`, `$$_VALUE`, `$$_NEW_VALUE`.
pub(crate) const EXCLAMATION_OBJECT_KEYS: (&str, &str, &str) = ("value", "$value", "newValue");

/// `Navigation` (`pre_define.js`: `NAVIGATION`). It takes the page coordinate too, plus
/// `isUserCreateStack` when the call supplied its own stack.
pub(crate) const COMPONENT_NAVIGATION: &str = "Navigation";
/// `CREATE_BIND_COMPONENT`: the two components whose `create` binds a child block itself.
pub(crate) const CREATE_BIND_COMPONENT: &[&str] = &["ListItemGroup", "Refresh"];
pub(crate) const _CONTAINER_READER: &str = "ContainerReader";
pub(crate) const OBSERVECOMPONENTCREATION2: &str = "observeComponentCreation2";
pub(crate) const ISINITIALRENDER: &str = "isInitialRender";
pub(crate) const ELMTID: &str = "elmtId";

// ── @AnimatableExtend ────────────────────────────────────────────────────────────────────
// `createAnimatableBody` / `createAnimatableProperty` (`process_ui_syntax.js`) build the body
// of an `@AnimatableExtend` function from these names.
pub(crate) const VIEW_STACK_PROCESSOR: &str = "ViewStackProcessor"; // VIEW_STACK_PROCESSOR
pub(crate) const STARTGETACCESSRECORDINGFOR: &str = "StartGetAccessRecordingFor";
pub(crate) const STOPGETACCESSRECORDING: &str = "StopGetAccessRecording";
pub(crate) const GET_AND_PUSH_FRAME_NODE: &str = "GetAndPushFrameNode";
pub(crate) const CREATE_ANIMATABLE_PROPERTY: &str = "createAnimatableProperty";
pub(crate) const UPDATE_ANIMATABLE_PROPERTY: &str = "updateAnimatableProperty";
pub(crate) const FINISH_UPDATE_FUNC: &str = "finishUpdateFunc";

// ── the class a struct becomes ───────────────────────────────────────────────────────────
pub(crate) const BASE_COMPONENT_NAME_PU: &str = "ViewPU"; // BASE_COMPONENT_NAME_PU
pub(crate) const BASE_COMPONENT_NAME: &str = "View";
pub(crate) const PUV2_VIEW_BASE: &str = "PUV2ViewBase";
pub(crate) const CUSTOM_COMPONENT: &str = "CustomComponent";
pub(crate) const INTERFACE_NAME_SUFFIX: &str = "_Params";
pub(crate) const FINALIZE_CONSTRUCTION: &str = "finalizeConstruction";
pub(crate) const COMPONENT_BUILD_FUNCTION: &str = "build";
pub(crate) const COMPONENT_INITIAL_RENDER_FUNCTION: &str = "initialRender";
pub(crate) const COMPONENT_RENDER_FUNCTION: &str = "render";
pub(crate) const COMPONENT_RERENDER_FUNCTION: &str = "rerender";
pub(crate) const COMPONENT_UPDATE_STATE_VARS: &str = "updateStateVars";
pub(crate) const COMPONENT_CONSTRUCTOR_INITIAL_PARAMS: &str = "setInitiallyProvidedValue";
pub(crate) const COMPONENT_CONSTRUCTOR_PURGE_VARIABLE_DEP: &str =
    "purgeVariableDependenciesOnElmtId";
pub(crate) const COMPONENT_CONSTRUCTOR_DELETE_PARAMS: &str = "aboutToBeDeleted";
pub(crate) const COMPONENT_CONSTRUCTOR_ID: &str = "compilerAssignedUniqueChildId";
pub(crate) const COMPONENT_CONSTRUCTOR_PARENT: &str = "parent";
pub(crate) const COMPONENT_CONSTRUCTOR_PARAMS: &str = "params";
pub(crate) const COMPONENT_CONSTRUCTOR_LOCALSTORAGE_PU: &str = "__localStorage";
pub(crate) const COMPONENT_CONSTRUCTOR_LOCALSTORAGE_TYPE_PU: &str = "LocalStorage";
pub(crate) const COMPONENT_CONSTRUCTOR_UPDATE_PARAMS: &str = "updateWithValueParams";
pub(crate) const COMPONENT_CONSTRUCTOR_UNDEFINED: &str = "undefined";
pub(crate) const COMPONENT_PARAMS_FUNCTION: &str = "paramsGenerator_";
pub(crate) const COMPONENT_PARAMS_LAMBDA_FUNCTION: &str = "paramsLambda";
pub(crate) const COMPONENT_ABOUTTOREUSEINTERNAL_FUNCTION: &str = "aboutToReuseInternal";
pub(crate) const CUSTOM_COMPONENT_FUNCTION_FIND_CHILD_BY_ID: &str = "findChildById";
pub(crate) const CUSTOM_COMPONENT_EARLIER_CREATE_CHILD: &str = "earlierCreatedChild_";
pub(crate) const GET_ENTRYNAME: &str = "getEntryName";
pub(crate) const COMPONENT_CALL: &str = "componentCall";

// ── observation backstores ───────────────────────────────────────────────────────────────
pub(crate) const OBSERVED_PROPERTY_SIMPLE_PU: &str = "ObservedPropertySimplePU";
pub(crate) const OBSERVED_PROPERTY_OBJECT_PU: &str = "ObservedPropertyObjectPU";
pub(crate) const SYNCHED_PROPERTY_SIMPLE_ONE_WAY_PU: &str = "SynchedPropertySimpleOneWayPU";
pub(crate) const SYNCHED_PROPERTY_OBJECT_ONE_WAY_PU: &str = "SynchedPropertyObjectOneWayPU";
pub(crate) const SYNCHED_PROPERTY_SIMPLE_TWO_WAY_PU: &str = "SynchedPropertySimpleTwoWayPU";
pub(crate) const SYNCHED_PROPERTY_OBJECT_TWO_WAY_PU: &str = "SynchedPropertyObjectTwoWayPU";
pub(crate) const SYNCHED_PROPERTY_NESED_OBJECT_PU: &str = "SynchedPropertyNesedObjectPU";
pub(crate) const OBSERVED_PROPERTY_ABSTRACT_PU: &str = "ObservedPropertyAbstractPU";
pub(crate) const CREATE_STORAGE_LINK: &str = "createStorageLink";
pub(crate) const CREATE_STORAGE_PROP: &str = "createStorageProp";
pub(crate) const CREATE_LOCAL_STORAGE_LINK: &str = "createLocalStorageLink";
pub(crate) const CREATE_LOCAL_STORAGE_PROP: &str = "createLocalStorageProp";
pub(crate) const APP_STORAGE: &str = "AppStorage";
pub(crate) const APP_STORAGE_SET_AND_PROP: &str = "SetAndProp";
pub(crate) const APP_STORAGE_SET_AND_LINK: &str = "SetAndLink";
pub(crate) const INITIALIZE_CONSUME_FUNCTION: &str = "initializeConsume";
pub(crate) const ADD_PROVIDED_VAR: &str = "addProvidedVar";
pub(crate) const CREATE_GET_METHOD: &str = "get";
pub(crate) const CREATE_SET_METHOD: &str = "set";
pub(crate) const RESERT: &str = "reset";
pub(crate) const RESET_SOURCE: &str = "resetSource";
pub(crate) const RESET_STATE_VARS_ON_REUSE: &str = "resetStateVarsOnReuse";
pub(crate) const IS_REUSABLE_: &str = "isReusable_";

// ── @CustomDialog construction ───────────────────────────────────────────────────────────
pub(crate) const SET_CONTROLLER_METHOD: &str = "setController"; // SET_CONTROLLER_METHOD
pub(crate) const SET_CONTROLLER_CTR: &str = "ctr"; // SET_CONTROLLER_CTR
pub(crate) const SET_CONTROLLER_CTR_TYPE: &str = "CustomDialogController";
pub(crate) const CUSTOM_DIALOG_CONTROLLER_BUILDER: &str = "builder";
pub(crate) const JS_DIALOG: &str = "jsDialog";

// ── file scaffolding ─────────────────────────────────────────────────────────────────────
pub(crate) const REGISTER_NAMED_ROUTE: &str = "registerNamedRoute"; // REGISTER_NAMED_ROUTE
pub(crate) const ROUTE_NAME: &str = "routeName";
pub(crate) const PAGE_PATH: &str = "pagePath";
pub(crate) const PAGE_FULL_PATH: &str = "pageFullPath";
pub(crate) const IS_USER_CREATE_STACK: &str = "isUserCreateStack";
pub(crate) const INTEGRATED_HSP: &str = "integratedHsp";
pub(crate) const BUILDER_ATTR_BIND: &str = "bind"; // BUILDER_ATTR_BIND
pub(crate) const BUILDER_PARAM_PROXY: &str = "makeBuilderParameterProxy";
pub(crate) const WRAPPEDBUILDER_CLASS: &str = "WrappedBuilder";
pub(crate) const MUTABLEBUILDER_CLASS: &str = "MutableBuilder";
pub(crate) const THIS: &str = "this";
pub(crate) const TRUE: &str = "true";
pub(crate) const FALSE: &str = "false";
pub(crate) const COMPONENT_COMMON: &str = "__Common__";
pub(crate) const MY_IDS: &str = "myIds";
pub(crate) const PUSH: &str = "push";

// ── call-site rewrites in `processUISyntax` ──────────────────────────────────────────────
/// `GLOBAL_CONTEXT` (`pre_define.js`): the receiver `processAnimateToOrImmediately` writes in
/// front of a bare `animateTo(…)` call.
pub(crate) const GLOBAL_CONTEXT: &str = "Context";
/// `ATTRIBUTE_ANIMATETO_SET` (`pre_define.js`): the bare calls rewritten to `Context.<name>(…)`.
pub(crate) const ATTRIBUTE_ANIMATETO_SET: &[&str] = &["animateTo", "animateToImmediately"];
/// `WORKER_OBJECT` (`pre_define.js`): the property name `processWorker` recognises on a
/// `new <namespace>.Worker(…)` construction.
pub(crate) const WORKER_OBJECT: &str = "Worker";
