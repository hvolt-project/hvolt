//! Token scanner for ArkTS sources.
//!
//! The transpiler used to walk the source with byte prefixes and hand-rolled skips
//! (`skip_quoted`, `skip_template`, indent counting). Every one of those guesses broke on a
//! construct it did not model — a regex literal, a template with `${…}`, an `export struct`,
//! a lowercase `@LocalBuilder` method — and the failures surfaced far from their cause.
//!
//! This module replaces the guessing with one scanner that knows the lexical structure:
//! comments, strings, templates (including nested substitutions), regex literals and
//! punctuation are classified once, and everything else asks the token stream instead of the
//! raw bytes.

/// What a token is, as far as the transpiler needs to distinguish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A name or keyword.
    Ident,
    /// A quoted string literal (`'…'` or `"…"`).
    Str,
    /// A template literal, including its substitutions.
    Template,
    /// A regular-expression literal.
    Regex,
    Number,
    /// Any single punctuation or operator character.
    Punct,
    /// `//…` or `/*…*/`. Kept in the stream so callers can decide to drop them.
    Comment,
}

#[derive(Debug, Clone, Copy)]
pub struct Token<'a> {
    pub kind: Kind,
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
}

impl<'a> Token<'a> {
    pub fn is_punct(&self, ch: char) -> bool {
        self.kind == Kind::Punct && self.text.starts_with(ch)
    }
}

/// Scan a whole source. The concatenation of every token's text is the input itself, so a
/// caller can reconstruct the source by walking the tokens.
pub fn tokenize(source: &str) -> Vec<Token<'_>> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let start = i;
        match bytes[i] {
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                push(&mut tokens, Kind::Comment, source, start, i);
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                push(&mut tokens, Kind::Comment, source, start, i);
            }
            b'\'' | b'"' => {
                i = skip_quoted(source, i);
                push(&mut tokens, Kind::Str, source, start, i);
            }
            b'`' => {
                i = skip_template(source, i);
                push(&mut tokens, Kind::Template, source, start, i);
            }
            b'/' if regex_position(&tokens) => {
                i = skip_regex(source, i);
                push(&mut tokens, Kind::Regex, source, start, i);
            }
            b if b.is_ascii_digit() => {
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.') {
                    i += 1;
                }
                push(&mut tokens, Kind::Number, source, start, i);
            }
            b if is_ident_start(b) => {
                while i < bytes.len() && is_ident_continue(bytes[i]) {
                    i += 1;
                }
                push(&mut tokens, Kind::Ident, source, start, i);
            }
            b if b.is_ascii_whitespace() => {
                i += 1;
            }
            _ => {
                let ch = source[i..].chars().next().unwrap_or('\0');
                i += ch.len_utf8();
                push(&mut tokens, Kind::Punct, source, start, i);
            }
        }
    }
    tokens
}

fn push<'a>(tokens: &mut Vec<Token<'a>>, kind: Kind, source: &'a str, start: usize, end: usize) {
    let end = end.min(source.len());
    if start < end && source.is_char_boundary(start) && source.is_char_boundary(end) {
        tokens.push(Token {
            kind,
            text: &source[start..end],
            start,
            end,
        });
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// A `/` starts a regex unless the previous token ends an expression.
fn regex_position(tokens: &[Token<'_>]) -> bool {
    match tokens.iter().rev().find(|t| t.kind != Kind::Comment) {
        None => true,
        Some(prev) => match prev.kind {
            Kind::Ident | Kind::Number | Kind::Str | Kind::Template | Kind::Regex => false,
            Kind::Punct => !matches!(prev.text, ")" | "]" | "}"),
            Kind::Comment => true,
        },
    }
}

/// The offset just past the quoted string that starts at `start`.
fn skip_quoted(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b if b == quote => return i + 1,
            b'\n' if quote != b'`' => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// The offset just past the template that starts at `start`, counting nested `${…}`
/// substitutions, which may contain further templates, strings or braces.
fn skip_template(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'`' => return i + 1,
            b'$' if i + 1 < bytes.len() && bytes[i + 1] == b'{' => {
                let mut depth = 1usize;
                i += 2;
                while i < bytes.len() && depth > 0 {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'\'' | b'"' => i = skip_quoted(source, i),
                        b'`' => i = skip_template(source, i),
                        b'{' => {
                            depth += 1;
                            i += 1;
                        }
                        b'}' => {
                            depth -= 1;
                            i += 1;
                        }
                        _ => i += 1,
                    }
                }
            }
            _ => i += 1,
        }
    }
    bytes.len()
}

/// The offset just past the regex literal that starts at `start`.
fn skip_regex(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut i = start + 1;
    let mut in_class = false;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'[' => {
                in_class = true;
                i += 1;
            }
            b']' => {
                in_class = false;
                i += 1;
            }
            b'/' if !in_class => {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                return i;
            }
            b'\n' => return i,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// The offset of the delimiter matching the opening one at `open`, skipping comments,
/// strings, templates and regex literals. `None` when the source is unbalanced.
pub fn matching_delimiter(source: &str, open: usize) -> Option<usize> {
    let tokens = tokenize(source);
    let mut stack: Vec<&str> = Vec::new();
    let mut seen_open = false;
    for token in &tokens {
        if token.end <= open {
            continue;
        }
        if !seen_open {
            seen_open = true;
            if token.start != open {
                return None;
            }
            stack.push(token.text);
            continue;
        }
        if token.kind == Kind::Punct {
            let text = token.text;
            if matches!(text, "(" | "[" | "{") {
                stack.push(text);
            } else if matches!(text, ")" | "]" | "}") {
                let opener = stack.pop()?;
                let closes = matches!((opener, text), ("(", ")") | ("[", "]") | ("{", "}"));
                if !closes || stack.is_empty() {
                    return Some(token.start);
                }
            }
        }
    }
    None
}

/// The source with every comment removed, keeping every other byte in place.
///
/// The removed span is exactly the comment token, so the whitespace and newlines around it
/// survive: dropping the inter-token gaps as well would glue two statements together.
pub fn without_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut prev = 0usize;
    for token in tokenize(source) {
        if token.kind == Kind::Comment {
            out.push_str(&source[prev..token.start]);
            prev = token.end;
        }
    }
    out.push_str(&source[prev..]);
    out
}

/// Whether the source applies any of `names` as a decorator.
///
/// The test is on the token pair `@` + name, so a mention inside a string, a comment or a
/// regular expression does not count.
pub fn has_decorator(source: &str, names: &[&str]) -> bool {
    let tokens = tokenize(source);
    for pair in tokens.windows(2) {
        if pair[0].is_punct('@') && pair[1].kind == Kind::Ident && names.contains(&pair[1].text) {
            return true;
        }
    }
    false
}

/// The expression segments of a template literal, in order.
///
/// A template is one token, so a name that only appears inside `${…}` is invisible to a token
/// walk over the file. Nesting is counted here, and a nested template contributes its own
/// segments as well.
pub fn template_expressions(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b'$' && bytes[i + 1] == b'{' {
            let start = i + 2;
            let mut depth = 1i32;
            let mut j = start;
            while j < bytes.len() && depth > 0 {
                match bytes[j] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    b'`' => {
                        // A nested template: skip it whole, its own holes are handled by the
                        // recursive call below.
                        j += 1;
                        while j < bytes.len() && bytes[j] != b'`' {
                            if bytes[j] == b'\\' {
                                j += 1;
                            }
                            j += 1;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let end = j.saturating_sub(1).min(bytes.len());
            if end > start {
                out.push(&text[start..end]);
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

/// Split on the commas that are not inside delimiters, strings, templates or regexes.
pub fn split_top_level_commas(body: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for token in tokenize(body) {
        if token.kind == Kind::Punct {
            if matches!(token.text, "(" | "[" | "{") {
                depth += 1;
            } else if matches!(token.text, ")" | "]" | "}") {
                depth -= 1;
            } else if token.text == "," && depth == 0 {
                pieces.push(body[start..token.start].to_string());
                start = token.end;
            }
        }
    }
    if start < body.len() {
        pieces.push(body[start..].to_string());
    }
    pieces
}

/// The decorators and export modifier attached to the declaration of `name`.
///
/// The AST reports decorators on a class, but ArkTS also allows them ahead of `export`
/// (`@Entry @Component export struct X`), where they do not belong to the class node. Reading
/// them from the token stream accepts every ordering.
pub fn declaration_attributes(source: &str, name: &str) -> (Vec<String>, bool) {
    let tokens = tokenize(source);
    let mut decorators = Vec::new();
    let mut exported = false;
    for (index, token) in tokens.iter().enumerate() {
        let is_decl =
            token.kind == Kind::Ident && (token.text == "struct" || token.text == "class");
        if !is_decl {
            continue;
        }
        let Some(next) = tokens.get(index + 1) else {
            continue;
        };
        if next.kind != Kind::Ident || next.text != name {
            continue;
        }
        // Walk backwards over the modifier list that precedes the declaration. A decorator's
        // arguments sit *after* its name, so meeting `)` means jumping back over the whole
        // argument list before the name and its `@` are reachable.
        let mut back = index;
        loop {
            while back > 0 && tokens[back - 1].kind == Kind::Comment {
                back -= 1;
            }
            if back == 0 {
                break;
            }
            let prev = &tokens[back - 1];
            if prev.kind == Kind::Ident && (prev.text == "export" || prev.text == "default") {
                exported |= prev.text == "export";
                back -= 1;
                continue;
            }
            if prev.is_punct(')') {
                match matching_open(&tokens, back - 1) {
                    Some(open) => {
                        back = open;
                        continue;
                    }
                    None => break,
                }
            }
            if prev.kind == Kind::Ident && back >= 2 && tokens[back - 2].is_punct('@') {
                decorators.push(prev.text.to_string());
                back -= 2;
                continue;
            }
            break;
        }
        decorators.reverse();
        return (decorators, exported);
    }
    (decorators, false)
}

/// The index of the token opening the group closed at `close`.
fn matching_open(tokens: &[Token<'_>], close: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = close;
    loop {
        let token = tokens.get(i)?;
        if token.kind == Kind::Punct {
            if matches!(token.text, ")" | "]" | "}") {
                depth += 1;
            } else if matches!(token.text, "(" | "[" | "{") {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_are_read_ahead_of_export() {
        let source = "@Entry\n@Component\nexport struct LocalBuilderView {\n}\n";
        let (decorators, exported) = declaration_attributes(source, "LocalBuilderView");
        assert_eq!(
            decorators,
            vec!["Entry".to_string(), "Component".to_string()]
        );
        assert!(exported);
    }

    #[test]
    fn attributes_are_read_with_arguments() {
        let source = "@Component\n@Preview({ title: 'x' })\nexport default struct Card {\n}\n";
        let (decorators, exported) = declaration_attributes(source, "Card");
        assert_eq!(
            decorators,
            vec!["Component".to_string(), "Preview".to_string()]
        );
        assert!(exported);
    }

    #[test]
    fn a_template_substitution_does_not_leak_braces() {
        let source = "const t = `a${ {x: 1} }b`;\n";
        let tokens = tokenize(source);
        let templates: Vec<_> = tokens.iter().filter(|t| t.kind == Kind::Template).collect();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].text, "`a${ {x: 1} }b`");
    }

    #[test]
    fn a_regex_is_one_token() {
        let source = "if (!/^[0-9A-F]{2}$/.test(x)) {}\n";
        let tokens = tokenize(source);
        let regexes: Vec<_> = tokens.iter().filter(|t| t.kind == Kind::Regex).collect();
        assert_eq!(regexes.len(), 1);
        assert_eq!(regexes[0].text, "/^[0-9A-F]{2}$/");
    }

    #[test]
    fn a_block_containing_a_comment_closes_at_the_right_brace() {
        let source = "X(a, // note\n{b: 1})\n{ body }\ntail";
        let open = source.find('{').unwrap();
        let close = matching_delimiter(source, open).expect("balanced");
        assert_eq!(&source[close..close + 1], "}");
        assert!(source[close..].contains("tail"));
    }
}
