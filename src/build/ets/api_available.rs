//! `deviceInfo.apiAvailable(v)`: an API-level check written as a call, compiled as a comparison.
//!
//! The runtime exposes the two version numbers the call compares against, so the check becomes
//! a relational expression over `sdkApiVersion` or `distributionOSApiVersion`. The rewrite only
//! applies to the `deviceInfo` the SDK declares — a local object that happens to have a method
//! named `apiAvailable` keeps its call (measured: `fixtures/api-available-shadow`).
//!
//! `process_available_statement.js` decides the four things this module reproduces:
//!
//! * `transformAvailableStatement` reads the single argument: a numeric literal is an API level
//!   (`isValidNumericVersion`: `^(?:[1-9]\d?)$` and 0 < v < 26), a string literal or a
//!   no-substitution template literal is a version, encoded by `convertToDistributeVersion` as
//!   `major + minor.padStart(2,'0') + patch.padStart(2,'0')` with an optional `(api)` suffix;
//! * `transformExpression` rewrites the *callee*, so an element access stays an element access
//!   and a `?.` chain keeps its chain: `deviceInfo['apiAvailable'](11)` becomes
//!   `deviceInfo["sdkApiVersion"] >= 11`, `deviceInfo?.apiAvailable(11)` becomes
//!   `deviceInfo?.sdkApiVersion >= 11`;
//! * the visitor is `ts.visitEachChild`, which descends into a template literal's `${…}`
//!   substitutions, so a call written inside one is rewritten as well;
//! * the whole rewrite is skipped when the project's `originCompatibleSdkVersion` is a point
//!   version (`isPointVersion`: 26.x and above).

use std::cell::RefCell;

use crate::build::lex::{matching_delimiter, tokenize, Kind, Token};

const SDK_VERSION: &str = "sdkApiVersion";
const DISTRIBUTION_VERSION: &str = "distributionOSApiVersion";

thread_local! {
    /// `projectConfig.originCompatibleSdkVersion` for the project being built.
    ///
    /// The reference reads it off its project config at the call site
    /// (`process_ui_syntax.js`: `!isPointVersion(projectConfig.originCompatibleSdkVersion)`);
    /// the lowering here is a pure text pass, so the pipeline publishes the value first. Until
    /// it does, the gate reads as "not a point version", which is the behaviour every fixture
    /// measured before the gate existed.
    static COMPATIBLE_VERSION: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Publish the project's compatible SDK version (`hvolt.toml`: `compatible_sdk_version`).
///
/// The pipeline is the caller (`deps/pipeline.rs`, next to `set_router_map`); until that one
/// line lands, the gate has no value to read and the rewrite keeps its pre-gate behaviour.
#[allow(dead_code)]
pub(crate) fn set_compatible_version(version: &str) {
    COMPATIBLE_VERSION.with(|slot| *slot.borrow_mut() = Some(version.to_string()));
}

/// Whether the published version is a point version, i.e. whether the rewrite is skipped.
fn point_version() -> bool {
    COMPATIBLE_VERSION.with(|slot| slot.borrow().as_deref().is_some_and(is_point_version))
}

/// `isPointVersion` (`process_available_statement.js`):
/// `^'?(?:2[6-9]|[3-9][0-9])\.(?:[0-9]|[1-9][0-9]?)\.(?:[0-9]|[1-9][0-9]?)(\(\d+\))?'?$`.
fn is_point_version(version: &str) -> bool {
    let text = version.trim_matches(|c| c == '\'' || c == '"');
    let core = match text.find('(') {
        Some(at) if text.ends_with(')') => {
            let inner = &text[at + 1..text.len() - 1];
            if inner.is_empty() || !inner.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
            &text[..at]
        }
        Some(_) => return false,
        None => text,
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    // `2[6-9]` or `[3-9][0-9]`: two digits, no leading zero, 26..=99.
    let major = parts[0];
    if major.len() != 2 || !major.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let major: u32 = major.parse().unwrap_or(0);
    if !(26..=99).contains(&major) {
        return false;
    }
    // `[0-9]` or `[1-9][0-9]?`: one digit, or two digits without a leading zero.
    parts[1..].iter().all(|part| {
        !part.is_empty()
            && part.len() <= 2
            && part.chars().all(|c| c.is_ascii_digit())
            && (part.len() == 1 || !part.starts_with('0'))
    })
}

/// Rewrite every `deviceInfo.apiAvailable(…)` call into the comparison it stands for.
pub(crate) fn rewrite(source: &str) -> String {
    if point_version() {
        return source.to_string();
    }
    rewrite_region(source)
}

/// One stretch of source: the file itself, or the body of a template substitution.
fn rewrite_region(text: &str) -> String {
    let tokens = tokenize(text);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        // `visitEachChild` walks into a template literal's substitutions; the lexer hands the
        // whole literal over as one token, so that walk is done here.
        if token.kind == Kind::Template {
            for (start, end) in substitutions(text, token) {
                let inner = rewrite_region(&text[start..end]);
                if inner != text[start..end] {
                    edits.push((start, end, inner));
                }
            }
            continue;
        }
        if token.kind != Kind::Ident || token.text != "deviceInfo" {
            continue;
        }
        // `x.deviceInfo.apiAvailable(…)` is a member of something else, not the SDK object.
        if index > 0 && (tokens[index - 1].is_punct('.') || tokens[index - 1].is_punct('?')) {
            continue;
        }
        let Some((access, open_index)) = accessor(&tokens, index) else {
            continue;
        };
        let open = tokens[open_index];
        let Some(close) = matching_delimiter(text, open.start) else {
            continue;
        };
        let Some((field, value)) = comparison(text[open.end..close].trim()) else {
            continue;
        };
        let receiver = &text[token.start..tokens[index + 1].start];
        let call = match access {
            Access::Dot => format!("{receiver}.{field} >= {value}"),
            Access::Chain => format!("{receiver}?.{field} >= {value}"),
            // `transformElementAccessExpression` keeps the element form and writes the field as
            // a string literal (`createStringLiteral`), which prints with double quotes.
            Access::Element => format!("{receiver}[\"{field}\"] >= {value}"),
        };
        edits.push((token.start, close + 1, call));
    }
    crate::build::ets_assemble::apply_replacements(text, &edits)
}

/// How the callee reaches `apiAvailable`.
#[derive(Clone, Copy)]
enum Access {
    /// `deviceInfo.apiAvailable`
    Dot,
    /// `deviceInfo?.apiAvailable`
    Chain,
    /// `deviceInfo['apiAvailable']`
    Element,
}

/// The access form and the index of the argument list's `(` in `tokens`.
fn accessor(tokens: &[Token<'_>], index: usize) -> Option<(Access, usize)> {
    let first = tokens.get(index + 1)?;
    // `deviceInfo.apiAvailable(`
    if first.is_punct('.') {
        let method = tokens.get(index + 2)?;
        let open = tokens.get(index + 3)?;
        if method.kind == Kind::Ident && method.text == "apiAvailable" && open.is_punct('(') {
            return Some((Access::Dot, index + 3));
        }
        return None;
    }
    // `deviceInfo?.apiAvailable(`: the lexer emits `?` and `.` as two punctuation tokens.
    if first.is_punct('?') {
        let dot = tokens.get(index + 2)?;
        let method = tokens.get(index + 3)?;
        let open = tokens.get(index + 4)?;
        if dot.is_punct('.')
            && method.kind == Kind::Ident
            && method.text == "apiAvailable"
            && open.is_punct('(')
        {
            return Some((Access::Chain, index + 4));
        }
        return None;
    }
    // `deviceInfo['apiAvailable'](`
    if first.is_punct('[') {
        let key = tokens.get(index + 2)?;
        let close = tokens.get(index + 3)?;
        let open = tokens.get(index + 4)?;
        if key.kind == Kind::Str
            && string_body(key.text) == Some("apiAvailable")
            && close.is_punct(']')
            && open.is_punct('(')
        {
            return Some((Access::Element, index + 4));
        }
    }
    None
}

/// The content spans of a template literal's `${…}` substitutions.
fn substitutions(text: &str, token: &Token<'_>) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = token.start;
    while i + 1 < token.end {
        if bytes[i] == b'$' && bytes[i + 1] == b'{' {
            let start = i + 2;
            let mut depth = 1i32;
            let mut j = start;
            while j < token.end {
                match bytes[j] {
                    b'\'' | b'"' | b'`' => {
                        j = skip_quoted(text, j, token.end);
                        continue;
                    }
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            if depth != 0 {
                break;
            }
            out.push((start, j));
            i = j + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// The offset just past the quoted run that starts at `at`, or `end` when it never closes.
fn skip_quoted(text: &str, at: usize, end: usize) -> usize {
    let bytes = text.as_bytes();
    let quote = bytes[at];
    let mut i = at + 1;
    while i < end {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    end
}

/// The version field and value an argument compares against.
fn comparison(argument: &str) -> Option<(&'static str, String)> {
    let tokens: Vec<Token<'_>> = tokenize(argument)
        .into_iter()
        .filter(|token| token.kind != Kind::Comment)
        .collect();
    // `transformAvailableStatement` reads one literal; anything else — a variable, an
    // expression, two arguments — keeps its call.
    let token = tokens.first().filter(|_| tokens.len() == 1)?;
    match token.kind {
        Kind::Number => {
            let level = api_level(token.text)?;
            Some((SDK_VERSION, level.to_string()))
        }
        Kind::Str => Some((DISTRIBUTION_VERSION, encoded(string_body(token.text)?)?)),
        // A no-substitution template literal is a string literal the reference also converts.
        Kind::Template if !token.text.contains('$') => {
            let body = token.text.strip_prefix('`')?.strip_suffix('`')?;
            Some((DISTRIBUTION_VERSION, encoded(body)?))
        }
        _ => None,
    }
}

/// `isValidNumericVersion`: one or two digits, no leading zero, and 0 < v < 26.
fn api_level(text: &str) -> Option<u32> {
    if text.is_empty() || text.len() > 2 || text.starts_with('0') {
        return None;
    }
    if !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let level: u32 = text.parse().ok()?;
    (level > 0 && level < 26).then_some(level)
}

/// `convertToDistributeVersion`: `<major>.<minor>.<patch>(<api>)` → the digits concatenated,
/// minor and patch padded to two places, the optional `(api)` suffix ignored.
fn encoded(version: &str) -> Option<String> {
    let text = version.trim();
    if text.is_empty() {
        return None;
    }
    let core = match text.find('(') {
        Some(at) if text.ends_with(')') => {
            let inner = &text[at + 1..text.len() - 1];
            if inner.is_empty() || !inner.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            &text[..at]
        }
        Some(_) => return None,
        None => text,
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    for part in &parts {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
    }
    Some(format!("{}{:0>2}{:0>2}", parts[0], parts[1], parts[2]))
}

/// The text between a string literal's quotes, or `None` when it carries an escape.
fn string_body(text: &str) -> Option<&str> {
    let quote = text.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let body = text.strip_prefix(quote)?.strip_suffix(quote)?;
    (!body.contains('\\')).then_some(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An API level becomes a comparison against `sdkApiVersion`.
    #[test]
    fn an_api_level_becomes_a_numeric_comparison() {
        let out = rewrite("if (deviceInfo.apiAvailable(11)) {}");
        assert_eq!(out, "if (deviceInfo.sdkApiVersion >= 11) {}");
    }

    /// A version string becomes the encoded comparison against `distributionOSApiVersion`.
    #[test]
    fn a_version_string_becomes_an_encoded_comparison() {
        let out = rewrite("if (deviceInfo.apiAvailable('5.0.5(17)')) {}");
        assert_eq!(out, "if (deviceInfo.distributionOSApiVersion >= 50005) {}");
    }

    /// The `(api)` suffix is optional in the reference's own pattern.
    #[test]
    fn a_version_string_without_the_api_suffix_is_encoded_too() {
        let out = rewrite("if (deviceInfo.apiAvailable('5.0.5')) {}");
        assert_eq!(out, "if (deviceInfo.distributionOSApiVersion >= 50005) {}");
    }

    /// The callee is rewritten, so the element form keeps its element form.
    #[test]
    fn an_element_access_keeps_the_element_form() {
        let out = rewrite("if (deviceInfo['apiAvailable'](11)) {}");
        assert_eq!(out, "if (deviceInfo[\"sdkApiVersion\"] >= 11) {}");
    }

    /// A `?.` chain keeps its chain.
    #[test]
    fn a_chain_keeps_its_chain() {
        let out = rewrite("if (deviceInfo?.apiAvailable(11)) {}");
        assert_eq!(out, "if (deviceInfo?.sdkApiVersion >= 11) {}");
    }

    /// The visitor descends into a template literal's substitutions.
    #[test]
    fn a_call_inside_a_template_substitution_is_rewritten() {
        let out = rewrite("const s = `${deviceInfo.apiAvailable(11)}`;");
        assert_eq!(out, "const s = `${deviceInfo.sdkApiVersion >= 11}`;");
    }

    /// A level outside 1..25 keeps its call: the reference reports the level as an error and
    /// does not rewrite it.
    #[test]
    fn a_level_outside_the_range_keeps_its_call() {
        let src = "if (deviceInfo.apiAvailable(26)) {}";
        assert_eq!(rewrite(src), src);
    }

    /// A method of the same name on another object keeps its call.
    #[test]
    fn another_objects_api_available_keeps_its_call() {
        let src = "if (this.probe.apiAvailable(11)) {}";
        assert_eq!(rewrite(src), src);
    }

    /// A 26.x compatible version makes the whole rewrite a no-op.
    #[test]
    fn a_point_version_skips_the_rewrite() {
        let src = "if (deviceInfo.apiAvailable(11)) {}";
        set_compatible_version("26.0.0(1)");
        assert_eq!(rewrite(src), src);
        set_compatible_version("5.0.5(17)");
        assert_eq!(rewrite(src), "if (deviceInfo.sdkApiVersion >= 11) {}");
        COMPATIBLE_VERSION.with(|slot| *slot.borrow_mut() = None);
    }

    /// The point-version pattern is the reference's own.
    #[test]
    fn the_point_version_pattern_matches_the_reference() {
        for version in ["26.0.0", "26.0.0(1)", "'26.1.2'", "30.10.10(2)"] {
            assert!(is_point_version(version), "{version}");
        }
        for version in ["5.0.5(17)", "25.0.0", "26.0", "26.01.0"] {
            assert!(!is_point_version(version), "{version}");
        }
    }
}
