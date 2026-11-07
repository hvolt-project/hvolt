//! Comments the lowering has to carry across a rewrite.
//!
//! A comment is not syntax, so the parser hands it back as a token rather than as part of a
//! declaration. The one place the lowering needs it is the comment written above `build()`:
//! the reference prints it above `initialRender()`, which is where the build body lands.

/// The comments written directly above `at`, in source order.
///
/// Only whitespace may separate a comment from the position — and one comment from the next — so
/// a comment that belongs to an earlier declaration is not picked up. Nothing is decided by line
/// arithmetic: a block comment and a line comment both end where the scanner says they do.
///
/// The whole run is returned because the reference prints all of it: a two-line block above a
/// component comes out twice, one line per line, in source order (`fixtures/comment-leading`,
/// markers `BLOCK_ONE`/`BLOCK_TWO`, `BLOCK_A`/`BLOCK_B` and the three-line `TRIO_*` block).
/// A comment block reindented to a target column.
///
/// The reference reprints a comment where it appends it: the first line takes `prefix`, and every
/// continuation line takes `prefix` + one space before its own `*` (`    /**` puts `* …` at five
/// spaces — `fixtures/proxy-group-item`). The source's own continuation indent never reaches the
/// output, so a block copied verbatim from an indented source would otherwise keep it.
pub(crate) fn reindent_block(comment: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(comment.len());
    let mut lines = comment.lines();
    if let Some(first) = lines.next() {
        out.push_str(first.trim_start().trim_end());
        for line in lines {
            out.push('\n');
            out.push_str(prefix);
            out.push(' ');
            out.push_str(line.trim_start());
        }
    }
    out
}

/// A block moved to the column `delta` puts it at, keeping the columns it was written with.
///
/// The first line carries no prefix: the caller places it (`format!("    {…}")` in
/// `emit_v1_field_declaration`), which is [`reindent_block`]'s contract too. A `*`-style block
/// keeps that function's layout — first line by the caller, every continuation `prefix` plus a
/// space. A block whose continuations carry no `*` keeps its own columns instead: a
/// commented-out declaration has to stay readable, so each line moves by `delta` rather than
/// collapsing to one column (`ConfigInfo.ts` keeps `/*@State` and its second line at four).
pub(crate) fn reindent_block_with_delta(comment: &str, prefix: &str, delta: isize) -> String {
    let mut lines = comment.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let rest: Vec<&str> = lines.collect();
    // The style is read from the first continuation line, never per line: a block that mixes
    // `*` lines with plain ones is laid out one way throughout.
    if rest
        .first()
        .is_some_and(|line| line.trim_start().starts_with('*'))
    {
        return reindent_block(comment, prefix);
    }
    let mut out = String::with_capacity(comment.len());
    out.push_str(first.trim_start().trim_end());
    for line in rest {
        out.push('\n');
        if line.trim().is_empty() {
            continue;
        }
        let own = line.len() - line.trim_start().len();
        let target = (own as isize + delta).max(0) as usize;
        for _ in 0..target {
            out.push(' ');
        }
        out.push_str(line.trim_start().trim_end());
    }
    out
}

pub(crate) fn leading_comments_before(text: &str, at: usize) -> Vec<String> {
    let tokens = crate::build::lex::tokenize(text);
    let at = at.min(text.len());
    let mut end = at;
    let mut found = Vec::new();
    while let Some(previous) = tokens.iter().rev().find(|t| t.end <= end) {
        if previous.kind != crate::build::lex::Kind::Comment {
            break;
        }
        if !text[previous.end..end].chars().all(char::is_whitespace) {
            break;
        }
        // A comment that follows a completed statement on its line is that statement's trailing
        // comment, not the next declaration's leading one: `const a = 1; // TRAILING` belongs to
        // the statement. A comment after a bare `}` is kept, because the reference hangs it on
        // the branch that follows (`} // 分支` / `else if (…)`, the `else` keyword sitting
        // outside the alternate's own span — `an_else_branch_comment_is_found_before_the_keyword`).
        let before = &text[line_start(text, previous.start)..previous.start];
        if before.trim_end().ends_with(|c: char| c != '}') {
            break;
        }
        found.push(text[previous.start..previous.end].to_string());
        end = previous.start;
    }
    found.reverse();
    found
}

/// The offset where the line holding `at` starts.
fn line_start(text: &str, at: usize) -> usize {
    text[..at.min(text.len())]
        .rfind('\n')
        .map_or(0, |newline| newline + 1)
}

#[cfg(test)]
mod tests {

    use super::*;

    /// A run of comments above the position is returned whole, in source order.
    #[test]
    fn the_whole_comment_block_is_returned_in_source_order() {
        let src = "/* A */\n// B\n// C\nText('x')\n";
        let at = src.find("Text(").unwrap();
        assert_eq!(
            leading_comments_before(src, at),
            vec![
                "/* A */".to_string(),
                "// B".to_string(),
                "// C".to_string()
            ]
        );
    }

    /// A comment written after code on its line belongs to that line, so it is not the leading
    /// comment of the next declaration (measured: `fixtures/trailing-comment`, marker
    /// `TRAILING_PLAIN`).
    #[test]
    fn a_trailing_comment_is_not_taken_as_a_leading_one() {
        let src = "const a = 1; // TRAILING\n// LEADING\nText('x')\n";
        let at = src.find("Text(").unwrap();
        assert_eq!(
            leading_comments_before(src, at),
            vec!["// LEADING".to_string()]
        );
    }

    /// The same comment written on the next line is still the declaration's own: the judge only
    /// rejects a comment that shares its line with code.
    #[test]
    fn a_comment_on_its_own_line_after_code_is_still_leading() {
        let src = "const a = 1;\n// LEADING\nText('x')\n";
        let at = src.find("Text(").unwrap();
        assert_eq!(
            leading_comments_before(src, at),
            vec!["// LEADING".to_string()]
        );
    }

    /// A comment separated from the position by code is not a leading comment.
    #[test]
    fn code_between_the_comment_and_the_position_breaks_the_run() {
        let src = "// A\nconst a = 1;\nText('x')\n";
        let at = src.find("Text(").unwrap();
        assert!(leading_comments_before(src, at).is_empty());
    }
}
