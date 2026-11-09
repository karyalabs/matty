//! Source rewrites that make a CommonMark view read a message the way
//! Mattermost shows it, for a view that parses Markdown on its own (the
//! gpuicn rich text view).

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::parse::options;

/// `text` with every soft line break written as a hard one (`\` before the
/// newline). Mattermost keeps the line breaks a message was typed with, so
/// `one\ntwo` shows on two lines, where CommonMark would join them. Code,
/// image descriptions and everything else stay as written.
pub fn hard_line_breaks(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut copied = 0;
    let mut in_image = 0usize;
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Image { .. }) => in_image += 1,
            Event::End(TagEnd::Image) => in_image = in_image.saturating_sub(1),
            Event::SoftBreak if in_image == 0 => {
                // The break's range can start with the line's trailing
                // spaces; the backslash goes right before the newline.
                let Some(newline) = text[range.clone()].find(['\n', '\r']) else {
                    continue;
                };
                let at = range.start + newline;
                out.push_str(&text[copied..at]);
                out.push('\\');
                copied = at;
            }
            _ => {}
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// `text` with a code fence that never closes ended at its first line that
/// starts like a closing fence but has text after it, such as
/// ```` ``` #783 ````, as Mattermost reads it. CommonMark keeps such a line
/// as code, so the rest of the message would show as code; the text after
/// the fence moves to its own line, below the block. A fence that does
/// close later is left alone.
pub fn close_fences_with_trailing_text(text: &str) -> String {
    let mut text = text.to_owned();
    // Each pass splits one line, and a split line is never a candidate again.
    while let Some((at, fence_len)) = unclosed_fence_split(&text) {
        let rest = text[at + fence_len..].trim_start_matches([' ', '\t']);
        let mut fixed = String::with_capacity(text.len() + 1);
        fixed.push_str(&text[..at + fence_len]);
        fixed.push('\n');
        fixed.push_str(rest);
        text = fixed;
    }
    text
}

/// The byte offset and length of the fence on the line to split, when a
/// top-level fence runs to the end of `text` without closing.
fn unclosed_fence_split(text: &str) -> Option<(usize, usize)> {
    /// A fence's offset and length on a line to split.
    type Split = (usize, usize);
    // The open fence's character and length, and the first line inside it
    // that looks like a closing fence followed by text.
    let mut open: Option<(char, usize, Option<Split>)> = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let body = line.trim_end_matches(['\n', '\r']);
        let indent = body.len() - body.trim_start_matches(' ').len();
        if indent > 3 {
            continue;
        }
        let trimmed = &body[indent..];
        let Some(fence_char) = trimmed.chars().next().filter(|c| matches!(c, '`' | '~')) else {
            continue;
        };
        let fence_len = trimmed.len() - trimmed.trim_start_matches(fence_char).len();
        if fence_len < 3 {
            continue;
        }
        let after = &trimmed[fence_len..];
        match &mut open {
            None => {
                // A backtick fence's info string cannot hold a backtick.
                if fence_char == '`' && after.contains('`') {
                    continue;
                }
                open = Some((fence_char, fence_len, None));
            }
            Some((open_char, open_len, candidate)) => {
                if fence_char != *open_char || fence_len < *open_len {
                    continue;
                }
                if after.trim().is_empty() {
                    open = None;
                } else if candidate.is_none() {
                    *candidate = Some((start + indent, fence_len));
                }
            }
        }
    }
    open.and_then(|(_, _, candidate)| candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_breaks_become_hard_breaks() {
        assert_eq!(hard_line_breaks("one\ntwo\nthree"), "one\\\ntwo\\\nthree");
        assert_eq!(hard_line_breaks("trailing \nspace"), "trailing \\\nspace");
        assert_eq!(hard_line_breaks("> quoted\n> lines"), "> quoted\\\n> lines");
        assert_eq!(hard_line_breaks("- item\n  more"), "- item\\\n  more");
    }

    #[test]
    fn breaks_that_are_not_soft_stay() {
        for text in [
            "one\n\ntwo",
            "hard  \nbreak",
            "hard\\\nbreak",
            "```\ncode\nlines\n```",
            "    indented\n    code",
            "- one\n- two",
            "# title\ntext",
            "![two\nlines](x.png)",
            "`code\nspan`",
        ] {
            assert_eq!(hard_line_breaks(text), text, "{text:?}");
        }
    }

    #[test]
    fn a_fence_with_text_after_it_closes_an_unclosed_block() {
        assert_eq!(
            close_fences_with_trailing_text(
                "Fixed in\n```\ngit cherry-pick 1a2b3c\n``` #783\nmerged"
            ),
            "Fixed in\n```\ngit cherry-pick 1a2b3c\n```\n#783\nmerged"
        );
        assert_eq!(
            close_fences_with_trailing_text("~~~\na\n~~~ done"),
            "~~~\na\n~~~\ndone"
        );
    }

    #[test]
    fn fences_that_close_are_left_alone() {
        for text in [
            "```\na\n```",
            "```\n``` not a close\n```",
            "````\n``` shorter\n````",
            "```rust\nfn x() {}\n```\nafter",
            "```\nnever closed",
            "    ``` indented code\n    ``` more",
        ] {
            assert_eq!(close_fences_with_trailing_text(text), text, "{text:?}");
        }
    }
}
