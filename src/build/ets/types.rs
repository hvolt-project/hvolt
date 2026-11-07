//! Small facts about ArkTS types and component names.
//!
//! These are the questions the lowering asks repeatedly: is this type a value type, which
//! observation class does a decorator select, is this name a built-in component, and how does a
//! name look once it is written into a string literal.

/// The observation class a decorated field's backstore uses.
///
/// The reference picks by decorator and by whether the declared type is a *simple* type
/// (`process_component_member.js`): `@State` on a `number` is an `ObservedPropertySimplePU`,
/// on an object an `ObservedPropertyObjectPU`.
pub(crate) fn observed_class(decorator: &str, is_simple: bool) -> &'static str {
    match (decorator, is_simple) {
        ("State" | "Provide" | "Consume", true) => "ObservedPropertySimplePU",
        ("State" | "Provide" | "Consume", false) => "ObservedPropertyObjectPU",
        ("Prop", true) => "SynchedPropertySimpleOneWayPU",
        ("Prop", false) => "SynchedPropertyObjectOneWayPU",
        ("Link", true) => "SynchedPropertySimpleTwoWayPU",
        ("Link", false) => "SynchedPropertyObjectTwoWayPU",
        ("ObjectLink", _) => "SynchedPropertyNesedObjectPU",
        // `@StorageLink` / `@StorageProp` / `@LocalStorageLink` / `@LocalStorageProp` name
        // their storage at construction instead of a parent, so the caller builds them
        // directly and never asks for a class name.
        _ => "",
    }
}

/// Whether a declared type is observed as a value rather than as an object.
///
/// The loader's `simpleTypes` list is `string`, `number`, `boolean` and enums; a union of
/// literals is treated as simple as well, because the runtime compares those by value.
pub(crate) fn is_simple_type(type_text: Option<&str>) -> bool {
    let Some(text) = type_text else {
        // No annotation: the initializer decides at run time, and the object form accepts
        // every value, so the wider type is the safe reading.
        return false;
    };
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    if matches!(text, "string" | "number" | "boolean") {
        return true;
    }
    // `'a' | 'b'`, `1 | 2`, `true | false`: a union of literals holds a value.
    let variants: Vec<&str> = text.split('|').map(str::trim).collect();
    !variants.is_empty()
        && variants.iter().all(|variant| {
            !variant.is_empty()
                && (variant.starts_with('\'')
                    || variant.starts_with('"')
                    || variant.chars().all(|c| c.is_ascii_digit() || c == '.')
                    || matches!(*variant, "true" | "false"))
        })
}

/// A name as the body of a string literal, escaped the way the reference's printer escapes it.
pub(crate) fn escape_string_literal(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// The ArkUI components known without reading the SDK's component table.
///
/// A call to one of these takes a child block, so the child-block rewrite needs the name
/// before the SDK has been consulted. The list is not the source of truth — the caller adds
/// the SDK's own component list when it has one — it only has to cover the components a
/// source may use when no SDK table is available.
/// True for a component the reference marks `atomic` in the SDK's component table.
///
/// An atomic component is created without a child stack, so the reference emits no
/// `pop()` after it (`process_component_build.js`: `isContainerComponent || NEEDPOP_COMPONENT`,
/// the only atomic entries of that set being `Blank` and `Search`).  The list is the SDK's
/// `components/*.json` set with `"atomic": true` (26.0.0).
pub(crate) fn is_atomic_component(name: &str) -> bool {
    const ATOMIC: &[&str] = &[
        "AbilityComponent",
        "AlphabetIndexer",
        "Animator",
        "ArcAlphabetIndexer",
        "Blank",
        "CalendarPicker",
        "Camera",
        "Circle",
        "Component3D",
        "ContentSlot",
        "Divider",
        "DotMatrix",
        "Ellipse",
        "EmbeddedComponent",
        "FormComponent",
        "FrictionMotion",
        "GeometryView",
        "Image",
        "ImageAnimator",
        "ImageSpan",
        "IndicatorComponent",
        "Line",
        "LoadingProgress",
        "LocationButton",
        "Marquee",
        "MediaCachedImage",
        "NodeContainer",
        "PageTransitionEnter",
        "PageTransitionExit",
        "Particle",
        "PasteButton",
        "Path",
        "PatternLock",
        "Polygon",
        "Polyline",
        "Progress",
        "Radio",
        "Rect",
        "RemoteWindow",
        "RichEditor",
        "RichText",
        "SaveButton",
        "ScrollMotion",
        "Search",
        "Slider",
        "Span",
        "SpringMotion",
        "SpringProp",
        "SymbolSpan",
        "SymbolGlyph",
        "TextArea",
        "TextInput",
        "UIExtensionComponent",
        "Video",
        "Web",
    ];
    ATOMIC.contains(&name)
}

pub(crate) fn builtin_component_names() -> &'static [String] {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        [
            "AbilityComponent",
            "AlphabetIndexer",
            "Blank",
            "Button",
            "Calendar",
            "CalendarPicker",
            "Canvas",
            "Checkbox",
            "CheckboxGroup",
            "Circle",
            "Column",
            "ColumnSplit",
            "Counter",
            "DataPanel",
            "DatePicker",
            "Divider",
            "EffectComponent",
            "Ellipse",
            "Flex",
            "FlowItem",
            "FolderStack",
            "FormComponent",
            "Gauge",
            "Grid",
            "GridCol",
            "GridItem",
            "GridRow",
            "Hyperlink",
            "Image",
            "ImageAnimator",
            "ImageSpan",
            "Line",
            "List",
            "ListItem",
            "ListItemGroup",
            "LoadingProgress",
            "Marquee",
            "Menu",
            "MenuItem",
            "MenuItemGroup",
            "NavDestination",
            "Navigation",
            "Navigator",
            "NavRouter",
            "Panel",
            "Path",
            "PatternLock",
            "Polygon",
            "Polyline",
            "Progress",
            "QRCode",
            "Radio",
            "Rating",
            "Rect",
            "Refresh",
            "RelativeContainer",
            "RemoteWindow",
            "RichText",
            "Row",
            "RowSplit",
            "Scroll",
            "ScrollBar",
            "Search",
            "Select",
            "Shape",
            "SideBarContainer",
            "Slider",
            "Span",
            "Stack",
            "Stepper",
            "StepperItem",
            "Swiper",
            "SymbolGlyph",
            "TabContent",
            "Tabs",
            "Text",
            "TextArea",
            "TextClock",
            "TextInput",
            "TextPicker",
            "TextTimer",
            "TimePicker",
            "Toggle",
            "Toolbar",
            "ToolbarItem",
            "Video",
            "WaterFlow",
            "Web",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The value/object split follows the loader's `simpleTypes`: a union of literals is a
    /// value, an object type is not.
    #[test]
    fn simple_types_are_strings_numbers_booleans_and_literal_unions() {
        assert!(is_simple_type(Some("string")));
        assert!(is_simple_type(Some("number")));
        assert!(is_simple_type(Some("boolean")));
        assert!(is_simple_type(Some("'a' | 'b'")));
        assert!(!is_simple_type(Some("Date")));
        assert!(!is_simple_type(Some("Array<string>")));
        assert!(!is_simple_type(None));
    }

    /// Every decorator that owns a backstore names one class, and the others name none.
    #[test]
    fn each_decorator_selects_its_observation_class() {
        assert_eq!(observed_class("State", true), "ObservedPropertySimplePU");
        assert_eq!(observed_class("State", false), "ObservedPropertyObjectPU");
        assert_eq!(
            observed_class("Link", true),
            "SynchedPropertySimpleTwoWayPU"
        );
        assert_eq!(
            observed_class("ObjectLink", false),
            "SynchedPropertyNesedObjectPU"
        );
        assert_eq!(observed_class("Watch", true), "");
    }

    /// A name written into a string literal is escaped, not concatenated raw.
    #[test]
    fn a_name_is_escaped_for_a_string_literal() {
        assert_eq!(escape_string_literal("plain"), "plain");
        assert_eq!(escape_string_literal("it's"), "it\\'s");
        assert_eq!(escape_string_literal("a\\b"), "a\\\\b");
    }
}
