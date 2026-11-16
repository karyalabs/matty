//! Source rewrites that make a CommonMark view read a message the way
//! Mattermost shows it, for a view that parses Markdown on its own (the
//! gpuicn rich text view).

use std::ops::Range;

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

/// `text` with each `**bold**` or `__bold__` that Mattermost shows bold but
/// CommonMark leaves as literal asterisks made readable to CommonMark.
/// Mattermost's parser takes everything between two `**` as bold, so
/// `** spaced **` and a closing `**` at the start of the next line both
/// count; CommonMark needs the delimiters to touch the words. The spaces
/// and line breaks just inside the delimiters move outside them, or go
/// when they would start or end a line, and a line the move starts with
/// a block marker (`1.`, `-`, `#`, `>`) has the marker escaped, so the
/// message keeps its blocks.
pub fn mattermost_strong(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut copied = 0;
    for (open, close) in unmatched_strong_pairs(text) {
        let inner = &text[open + 2..close];
        let content = inner.trim();
        if content.is_empty() || content.len() == inner.len() {
            continue;
        }
        let leading = &inner[..inner.len() - inner.trim_start().len()];
        let trailing = &inner[inner.trim_end().len()..];
        let marker = &text[open..open + 2];
        out.push_str(&text[copied..open]);
        let space_before = text[..open].ends_with(char::is_whitespace);
        if !at_line_start(text, open) && (leading.contains('\n') || !space_before) {
            out.push_str(leading);
        }
        out.push_str(marker);
        out.push_str(content);
        out.push_str(marker);
        copied = close + 2;
        let space_after = text[copied..].starts_with(char::is_whitespace);
        if !at_line_end(text, copied) && (trailing.contains('\n') || !space_after) {
            out.push_str(trailing);
            if trailing.contains('\n') {
                copied += escape_block_start(&text[copied..], &mut out);
            }
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// The byte offsets of each pair of `**` or `__` that CommonMark leaves as
/// text but Mattermost reads as bold, paired in order within one run of
/// inline text (a paragraph, heading or table cell, outside any link or
/// emphasis), the way Mattermost pairs them.
fn unmatched_strong_pairs(text: &str) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    // The text of the current block: each run's range and its container,
    // the count of inline starts seen when the run's parent opened.
    let mut runs: Vec<(Range<usize>, usize)> = Vec::new();
    let mut containers = vec![0usize];
    let mut starts = 0usize;
    let mut in_inline_block = false;
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Paragraph | Tag::Heading { .. } | Tag::TableCell) => {
                in_inline_block = true;
                runs.clear();
                containers.truncate(1);
            }
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::TableCell) => {
                in_inline_block = false;
                pair_strong_delimiters(text, &runs, &mut pairs);
            }
            Event::Start(
                Tag::Emphasis
                | Tag::Strong
                | Tag::Strikethrough
                | Tag::Link { .. }
                | Tag::Image { .. },
            ) => {
                starts += 1;
                containers.push(starts);
            }
            Event::End(
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::Image,
            ) => {
                if containers.len() > 1 {
                    containers.pop();
                }
            }
            Event::Text(_) if in_inline_block => {
                runs.push((range, containers.last().copied().unwrap_or_default()));
            }
            _ => {}
        }
    }
    pairs
}

/// Pairs, in order, the `**` and `__` in a block's text `runs` that sit in
/// the same container.
fn pair_strong_delimiters(
    text: &str,
    runs: &[(Range<usize>, usize)],
    pairs: &mut Vec<(usize, usize)>,
) {
    let container_of = |at: usize| {
        runs.iter()
            .find(|(range, _)| range.contains(&at))
            .map(|(_, container)| *container)
    };
    let mut open: Option<(usize, u8, usize)> = None;
    for (range, container) in runs {
        for at in strong_delimiters(text, range.clone()) {
            // CommonMark can split a run of delimiters into several texts;
            // all of it must be text, in this container.
            if container_of(at + 1) != Some(*container) {
                continue;
            }
            let marker = text.as_bytes()[at];
            match open {
                Some((start, open_marker, open_container))
                    if open_marker == marker && open_container == *container =>
                {
                    pairs.push((start, at));
                    open = None;
                }
                _ => open = Some((at, marker, *container)),
            }
        }
    }
}

/// The offsets in `range` of `text` where an unescaped run of exactly two
/// `*` or `_` starts. A `__` must follow a space or punctuation:
/// Mattermost, like CommonMark, keeps `snake__case__name` as written.
fn strong_delimiters(text: &str, range: Range<usize>) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    for at in range {
        let byte = bytes[at];
        if (byte != b'*' && byte != b'_') || (at > 0 && bytes[at - 1] == byte) {
            continue;
        }
        let run = bytes[at..].iter().take_while(|&&b| b == byte).count();
        let escaped = bytes[..at]
            .iter()
            .rev()
            .take_while(|&&b| b == b'\\')
            .count()
            % 2
            == 1;
        let follows_word = text[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric());
        if run == 2 && !escaped && !(byte == b'_' && follows_word) {
            found.push(at);
        }
    }
    found
}

/// Whether only spaces come before `at` on its line.
fn at_line_start(text: &str, at: usize) -> bool {
    text[..at]
        .rsplit('\n')
        .next()
        .is_some_and(|line| line.trim().is_empty())
}

/// Whether only spaces come after `at` on its line.
fn at_line_end(text: &str, at: usize) -> bool {
    text[at..]
        .split('\n')
        .next()
        .is_some_and(|line| line.trim().is_empty())
}

/// Copies the start of `line` to `out` with a block marker it opens with
/// escaped, so a line that used to follow `**` stays paragraph text.
/// Returns how many bytes of `line` it copied.
fn escape_block_start(line: &str, out: &mut String) -> usize {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = &line[indent..];
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let marker_at = match rest.as_bytes().get(digits) {
        Some(b'.' | b')') if digits > 0 => indent + digits,
        Some(b'#' | b'>' | b'-' | b'+' | b'*' | b'=' | b'_' | b'|' | b'<' | b'`' | b'~')
            if digits == 0 =>
        {
            indent
        }
        _ => return 0,
    };
    out.push_str(&line[..marker_at]);
    out.push('\\');
    marker_at
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
    fn bold_with_spaces_inside_reads_as_mattermost_shows_it() {
        for (text, fixed) in [
            ("** spaced bold **", "**spaced bold**"),
            ("say **hi ** now", "say **hi** now"),
            ("say ** hi** now", "say **hi** now"),
            ("say**hi **now", "say**hi** now"),
            ("__ under __ line", "__under__ line"),
            (
                "**Release notes\n** 1. Faster search",
                "**Release notes**\n 1\\. Faster search",
            ),
            ("**Plan\n** - ship it", "**Plan**\n \\- ship it"),
            ("**Plan\n** # one", "**Plan**\n \\# one"),
            ("**Plan\n** later", "**Plan**\n later"),
            ("**Plan \n**\nnext", "**Plan**\nnext"),
            ("> ** quoted **", "> **quoted**"),
            ("| a |\n|---|\n| ** cell ** |", "| a |\n|---|\n| **cell** |"),
            ("**één **en", "**één** en"),
        ] {
            assert_eq!(mattermost_strong(text), fixed, "{text:?}");
        }
    }

    #[test]
    fn bold_that_commonmark_reads_or_mattermost_skips_stays() {
        for text in [
            "**bold** and __bold__",
            "**open only",
            "** **",
            "snake__case __name",
            "a\\** b **",
            "`** code **`",
            "```\n** code **\n```",
            "***both***",
            "[** link](x) **",
            "**Note:**text",
        ] {
            assert_eq!(mattermost_strong(text), text, "{text:?}");
        }
    }

    #[test]
    fn the_rewrite_makes_commonmark_see_bold() {
        let text = mattermost_strong("**Release notes\n** 1. Faster search");
        let events: Vec<_> = Parser::new_ext(&text, options()).collect();
        assert!(events.contains(&Event::Start(Tag::Strong)), "{events:?}");
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Start(Tag::List(_)))),
            "{events:?}"
        );
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
