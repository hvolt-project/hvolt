//! Chain-ending child blocks: `Column().width(1).justifyContent(Center) { … }`.
//!
//! ArkUI accepts the trailing block after a property chain; a TypeScript parser does not and
//! reports `Expected a semicolon` at the `{` (`entry/pages/Index.ets`). The block belongs to
//! the chain's first component call, so moving it there —
//! `Column() { … }.width(1).justifyContent(Center)` — is the same program and parses.

use crate::build::lex::{tokenize, Kind, Token};

/// Move every property chain's trailing child block back behind its component call.
pub(crate) fn rewrite_chain_blocks(source: &str) -> String {
    let tokens = tokenize(source);
    let mut out = String::with_capacity(source.len());
    let mut copied = 0usize;
    let mut hits = 0usize;
    let mut i = 0usize;
    while i + 1 < tokens.len() {
        let head = &tokens[i];
        let is_component = head.kind == Kind::Ident
            && head.text.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && tokens[i + 1].kind == Kind::Punct
            && tokens[i + 1].text == "(";
        if !is_component {
            i += 1;
            continue;
        }
        let Some(after_args) = match_close(&tokens, i + 1) else {
            i += 1;
            continue;
        };
        let mut cursor = after_args;
        loop {
            if cursor + 2 < tokens.len()
                && tokens[cursor].kind == Kind::Punct
                && tokens[cursor].text == "."
                && tokens[cursor + 1].kind == Kind::Ident
                && tokens[cursor + 2].kind == Kind::Punct
                && tokens[cursor + 2].text == "("
            {
                match match_close(&tokens, cursor + 2) {
                    Some(next) => cursor = next,
                    None => break,
                }
                continue;
            }
            break;
        }
        if cursor >= tokens.len() || tokens[cursor].kind != Kind::Punct || tokens[cursor].text != "{" {
            i += 1;
            continue;
        }
        let Some(block_end) = match_close(&tokens, cursor) else {
            i += 1;
            continue;
        };
        let head_end = tokens[after_args - 1].end;
        let block_start = tokens[cursor].start;
        let block_stop = tokens[block_end - 1].end;
        if block_start < head_end || block_stop > source.len() {
            i += 1;
            continue;
        }
        let gap_raw = &source[head_end..block_start];
        let trailing = gap_raw.len() - gap_raw.trim_end().len();
        let (chain, gap) = gap_raw.split_at(gap_raw.len() - trailing);
        let body = &source[block_start..block_stop];
        out.push_str(&source[copied..head.start]);
        out.push_str(&source[head.start..head_end]);
        out.push_str(if gap.is_empty() { " " } else { gap });
        out.push_str(body);
        out.push_str(chain);
        copied = block_stop;
        hits += 1;
        i = block_end;
    }
    if std::env::var("HVOLT_ARKUI_DEBUG").is_ok() {
        use std::io::Write;
        if let Ok(mut log) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/hvolt-arkui.log")
        {
            let mut cols = Vec::new();
            for (idx, tok) in tokens.iter().enumerate() {
                if tok.text == "Column" {
                    let tail: Vec<String> = tokens[idx..(idx + 4).min(tokens.len())]
                        .iter()
                        .map(|t| format!("{:?}({})", t.kind, t.text))
                        .collect();
                    cols.push(format!("{idx}:{tail:?}"));
                }
            }
            let after = format!("{:?}", &out[out.len().saturating_sub(2000)..]);
            let _ = writeln!(
                log,
                "hits={hits} in={} tokens={} columns={cols:?}\nTAIL={after}",
                source.len(),
                tokens.len()
            );
        }
    }
    out.push_str(&source[copied..]);
    out
}

fn match_close(tokens: &[Token<'_>], open: usize) -> Option<usize> {
    let (want, close) = match tokens[open].text {
        "(" => ("(", ")"),
        "{" => ("{", "}"),
        "[" => ("[", "]"),
        _ => return None,
    };
    let mut depth = 0usize;
    for (offset, token) in tokens[open..].iter().enumerate() {
        if token.kind != Kind::Punct {
            continue;
        }
        if token.text == want {
            depth += 1;
        } else if token.text == close {
            depth -= 1;
            if depth == 0 {
                return Some(open + offset + 1);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chain_block_moves_behind_the_component_call() {
        let src = "Column()\n  .width(10)\n  .justifyContent(FlexAlign.Center) {\n    Text('x')\n  }\n";
        let out = rewrite_chain_blocks(src);
        assert_eq!(
            out,
            "Column() {\n    Text('x')\n  }\n  .width(10)\n  .justifyContent(FlexAlign.Center)\n"
        );
    }

    #[test]
    fn a_plain_component_block_is_untouched() {
        let src = "Row() {\n  Text('x')\n}\n";
        assert_eq!(rewrite_chain_blocks(src), src);
    }

    #[test]
    fn a_lowercase_call_keeps_its_block() {
        let src = "if (x) {\n  y()\n}\n";
        assert_eq!(rewrite_chain_blocks(src), src);
    }
}
