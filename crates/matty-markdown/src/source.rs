//! Entities as `matty://` links in the Markdown source itself, for a view
//! that parses Markdown on its own (the gpuicn rich text view, PLAN §6.8).
//!
//! [`link_entities`](crate::link_entities) rewrites the AST; this rewrites
//! the text, so `@ana` becomes `[@ana](matty://user/ana)` and everything else
//! stays byte for byte as written. The same rules decide what is an entity:
//! text inside code, links, images and bare URLs is left alone.

use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::autolink::link_spans;
use crate::entities::find;
use crate::parse::options;

/// `text` with its mentions, channels and hashtags written as Markdown links
/// to their `matty://` URLs, and the emoji `is_emoji` knows as
/// `![:name:](emoji:name)` images, the form gpuicn's `CustomEmoji` draws.
pub fn link_entities_in_source(text: &str, is_emoji: &dyn Fn(&str) -> bool) -> String {
    link_entities_and_emoji_in_source(text, is_emoji, &|_| None)
}

/// [`link_entities_in_source`], with the emoji `glyph` knows (system emoji,
/// such as `:clown_face:`) written as their glyph instead of an image link.
/// `is_emoji` must know them too.
pub fn link_entities_and_emoji_in_source(
    text: &str,
    is_emoji: &dyn Fn(&str) -> bool,
    glyph: &dyn Fn(&str) -> Option<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for run in text_runs(text) {
        let run_text = &text[run.clone()];
        let urls = link_spans(run_text);
        for entity in find(run_text, is_emoji) {
            if urls
                .iter()
                .any(|url| entity.start < url.end && url.start < entity.end)
            {
                continue;
            }
            let start = run.start + entity.start;
            let end = run.start + entity.end;
            // `\@ana`: the writer escaped it, so it stays text.
            if text[..start].ends_with('\\') {
                continue;
            }
            out.push_str(&text[copied..start]);
            let emoji = entity.url.strip_prefix("matty://emoji/");
            if let Some(glyph) = emoji.and_then(glyph) {
                out.push_str(&glyph);
                copied = end;
                continue;
            }
            if emoji.is_some() {
                out.push('!');
            }
            out.push('[');
            for c in text[start..end].chars() {
                if matches!(c, '\\' | '*' | '_' | '[' | ']' | '`') {
                    out.push('\\');
                }
                out.push(c);
            }
            out.push_str("](");
            match emoji {
                Some(name) => {
                    out.push_str("emoji:");
                    out.push_str(name);
                }
                None => out.push_str(&entity.url),
            }
            out.push(')');
            copied = end;
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// `source` with each `~name` channel link that [`link_entities_in_source`]
/// wrote labeled `~Display Name` instead, the way Mattermost shows a
/// channel mention, for the channels `display_name` knows. A link the
/// writer labeled themselves, or an unknown channel, stays as it is.
pub fn label_channel_links(source: &str, display_name: &dyn Fn(&str) -> Option<String>) -> String {
    const TARGET: &str = "](matty://channel/";
    let mut out = String::with_capacity(source.len());
    let mut copied = 0;
    let mut from = 0;
    while let Some(found) = source[from..].find(TARGET) {
        let close = from + found;
        let name_start = close + TARGET.len();
        from = name_start;
        let Some(name_len) = source[name_start..].find(')') else {
            break;
        };
        let name = &source[name_start..name_start + name_len];
        let Some(open) = source[copied..close].rfind("[~").map(|at| copied + at) else {
            continue;
        };
        let label = &source[open + 2..close];
        if label.is_empty()
            || !label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '\\'))
        {
            continue;
        }
        let Some(shown) = display_name(name) else {
            continue;
        };
        out.push_str(&source[copied..open + 2]);
        for c in shown.chars() {
            if matches!(c, '\\' | '*' | '_' | '[' | ']' | '`' | '<' | '>' | '~') {
                out.push('\\');
            }
            out.push(c);
        }
        copied = close;
    }
    out.push_str(&source[copied..]);
    out
}

/// `text` (Markdown source) with each of `words` written as a link to
/// `mark:`, for a view to draw highlighted: the reader's keywords in a
/// message, or the terms of a search among its results. A word matches
/// whole and in any case; one ending in `*` matches as a prefix, and one
/// with spaces matches as a phrase. Text inside code, links (mentions
/// included), images and bare URLs is left alone.
pub fn mark_words_in_source(text: &str, words: &[String]) -> String {
    let words: Vec<(Vec<char>, bool)> = words
        .iter()
        .filter_map(|word| {
            let (word, prefix) = match word.strip_suffix('*') {
                Some(stem) => (stem, true),
                None => (word.as_str(), false),
            };
            let chars: Vec<char> = word.trim().chars().flat_map(char::to_lowercase).collect();
            (!chars.is_empty()).then_some((chars, prefix))
        })
        .collect();
    if words.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + 16);
    let mut copied = 0;
    for run in text_runs(text) {
        let run_text = &text[run.clone()];
        let urls = link_spans(run_text);
        for found in word_matches(run_text, &words) {
            if urls
                .iter()
                .any(|url| found.start < url.end && url.start < found.end)
            {
                continue;
            }
            let start = run.start + found.start;
            let end = run.start + found.end;
            out.push_str(&text[copied..start]);
            out.push('[');
            for c in text[start..end].chars() {
                if matches!(c, '\\' | '*' | '_' | '[' | ']' | '`') {
                    out.push('\\');
                }
                out.push(c);
            }
            out.push_str("](mark:)");
            copied = end;
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// The byte ranges in `text` where one of `words` (lowercase, and whether
/// each is a prefix) stands as a word, earliest first, without overlaps;
/// at one place the longest match wins.
fn word_matches(text: &str, words: &[(Vec<char>, bool)]) -> Vec<Range<usize>> {
    let word_char = |c: char| c.is_alphanumeric() || c == '_';
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let starts = at == 0 || !word_char(chars[at - 1].1);
        let longest = starts
            .then(|| {
                words
                    .iter()
                    .filter_map(|(word, prefix)| {
                        let end = at + word.len();
                        let matched = chars.get(at..end)?.iter().zip(word).all(|((_, c), w)| {
                            let mut lower = c.to_lowercase();
                            lower.next() == Some(*w) && lower.next().is_none()
                        });
                        if !matched {
                            return None;
                        }
                        let mut end = end;
                        if *prefix {
                            while chars.get(end).is_some_and(|(_, c)| word_char(*c)) {
                                end += 1;
                            }
                        } else if chars.get(end).is_some_and(|(_, c)| word_char(*c)) {
                            return None;
                        }
                        Some(end)
                    })
                    .max()
            })
            .flatten();
        match longest {
            Some(end) => {
                let byte_end = chars.get(end).map_or(text.len(), |(index, _)| *index);
                found.push(chars[at].0..byte_end);
                at = end;
            }
            None => at += 1,
        }
    }
    found
}

/// Mattermost's emoticons and the emoji each one shows as, longest first
/// where one starts another. Letters match in either case.
const EMOTICONS: &[(&str, &str)] = &[
    (":-)", "slightly_smiling_face"),
    (":)", "slightly_smiling_face"),
    (";-)", "wink"),
    (";)", "wink"),
    (":-o", "scream"),
    (":o", "open_mouth"),
    (":-]", "smirk"),
    (":]", "smirk"),
    (":-d", "smile"),
    (":d", "smile"),
    ("x-d", "stuck_out_tongue_closed_eyes"),
    (":-p", "stuck_out_tongue"),
    (":p", "stuck_out_tongue"),
    (":-[", "rage"),
    (":[", "rage"),
    (":-@", "rage"),
    (":@", "rage"),
    (":`-(", "cry"),
    (":'-(", "cry"),
    (":\u{2019}-(", "cry"),
    (":`(", "cry"),
    (":'(", "cry"),
    (":\u{2019}(", "cry"),
    (":-(", "slightly_frowning_face"),
    (":(", "slightly_frowning_face"),
    (":-/", "confused"),
    (":/", "confused"),
    (":-s", "confounded"),
    (":s", "confounded"),
    (":-|", "neutral_face"),
    (":|", "neutral_face"),
    (":-$", "flushed"),
    (":$", "flushed"),
    (":-x", "mask"),
    ("</3", "broken_heart"),
    ("<3", "heart"),
];

/// `text` with its emoticons, such as `:)` and `<3`, written as the
/// `:name:` codes of the emoji Mattermost shows for them. As there, one
/// counts only between non-word characters, and code, links and URLs are
/// left alone.
pub fn emoticons_as_emoji_codes(text: &str) -> String {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for run in text_runs(text) {
        let run_text = &text[run.clone()];
        let urls = link_spans(run_text);
        let mut at = 0;
        while at < run_text.len() {
            let rest = &run_text[at..];
            let after_word = run_text[..at].chars().next_back().is_some_and(word);
            let found = (!after_word)
                .then(|| {
                    EMOTICONS.iter().find(|(emoticon, _)| {
                        rest.get(..emoticon.len())
                            .is_some_and(|head| head.eq_ignore_ascii_case(emoticon))
                            && !rest[emoticon.len()..].chars().next().is_some_and(word)
                    })
                })
                .flatten()
                .filter(|(emoticon, _)| {
                    let end = at + emoticon.len();
                    !urls.iter().any(|url| at < url.end && url.start < end)
                });
            match found {
                Some((emoticon, name)) => {
                    let start = run.start + at;
                    out.push_str(&text[copied..start]);
                    out.push(':');
                    out.push_str(name);
                    out.push(':');
                    at += emoticon.len();
                    copied = run.start + at;
                }
                None => at += rest.chars().next().map_or(1, char::len_utf8),
            }
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// The source ranges of plain text outside code, links and images, where
/// the source reads exactly as the text (no escapes or character
/// references), joining adjacent text events.
fn text_runs(text: &str) -> Vec<Range<usize>> {
    let mut runs = Vec::new();
    let mut current: Option<(Range<usize>, String)> = None;
    let mut inside = 0usize;
    let finish = |run: Option<(Range<usize>, String)>, runs: &mut Vec<Range<usize>>| {
        if let Some((range, decoded)) = run
            && text[range.clone()] == decoded
        {
            runs.push(range);
        }
    };
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Text(decoded) if inside == 0 => match &mut current {
                Some((run, joined)) if run.end == range.start => {
                    run.end = range.end;
                    joined.push_str(&decoded);
                }
                _ => {
                    finish(current.take(), &mut runs);
                    current = Some((range, decoded.into_string()));
                }
            },
            other => {
                finish(current.take(), &mut runs);
                match other {
                    Event::Start(Tag::Link { .. } | Tag::Image { .. } | Tag::CodeBlock(_)) => {
                        inside += 1;
                    }
                    Event::End(TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock) => {
                        inside = inside.saturating_sub(1);
                    }
                    _ => {}
                }
            }
        }
    }
    finish(current.take(), &mut runs);
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marked_words_are_whole_words_outside_code_and_links() {
        let words = |list: &[&str]| list.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            mark_words_in_source("Deploy the deployer, deploy!", &words(&["deploy"])),
            "[Deploy](mark:) the deployer, [deploy](mark:)!"
        );
        assert_eq!(
            mark_words_in_source("deploying and deploys", &words(&["deploy*"])),
            "[deploying](mark:) and [deploys](mark:)"
        );
        assert_eq!(
            mark_words_in_source(
                "the staging server is down",
                &words(&["staging server", "staging"])
            ),
            "the [staging server](mark:) is down"
        );
        for text in [
            "`deploy` and\n```\ndeploy\n```",
            "[deploy](https://example.com)",
            "https://example.com/deploy",
            "[@deploy](matty://user/deploy)",
        ] {
            assert_eq!(mark_words_in_source(text, &words(&["deploy"])), text);
        }
        assert_eq!(
            mark_words_in_source("**Größe** and größer", &words(&["GRÖSSE", "größe"])),
            "**[Größe](mark:)** and größer"
        );
        assert_eq!(mark_words_in_source("a_b", &words(&["_"])), "a_b");
        assert_eq!(
            mark_words_in_source("snake_case", &words(&["snake_case"])),
            "[snake\\_case](mark:)"
        );
    }

    #[test]
    fn emoticons_become_emoji_codes_between_words_only() {
        assert_eq!(
            emoticons_as_emoji_codes("hi :) ok :-D ;) <3 </3 :'( X-D :P"),
            "hi :slightly_smiling_face: ok :smile: :wink: :heart: :broken_heart: :cry: \
             :stuck_out_tongue_closed_eyes: :stuck_out_tongue:"
        );
        assert_eq!(
            emoticons_as_emoji_codes("(nice :))"),
            "(nice :slightly_smiling_face:)"
        );
        for text in [
            "a:) :pizza: :ok_hand: :Dx",
            "C:/path and https://example.com/a:)b",
            "`:)` and\n```\n:(\n```",
            "[:)](https://example.com)",
        ] {
            assert_eq!(emoticons_as_emoji_codes(text), text);
        }
    }

    fn rewrite(text: &str) -> String {
        link_entities_in_source(text, &|name| name == "tada")
    }

    #[test]
    fn channel_links_show_display_names() {
        let names = |name: &str| (name == "town-square").then(|| "Town Square [HQ]".to_owned());
        assert_eq!(
            label_channel_links(
                &rewrite("see ~Town-Square, ~off-topic and [here](matty://channel/town-square)"),
                &names
            ),
            "see [~Town Square \\[HQ\\]](matty://channel/town-square), \
             [~off-topic](matty://channel/off-topic) and [here](matty://channel/town-square)"
        );
    }

    #[test]
    fn entities_become_links() {
        assert_eq!(
            rewrite("hi @ana, see ~town-square and #release :tada:"),
            "hi [@ana](matty://user/ana), see [~town-square](matty://channel/town-square) \
             and [#release](matty://hashtag/release) ![:tada:](emoji:tada)"
        );
    }

    #[test]
    fn system_emoji_become_glyphs() {
        let glyph = |name: &str| match name {
            "clown_face" => Some("🤡".to_owned()),
            "+1_medium_skin_tone" => Some("👍🏽".to_owned()),
            _ => None,
        };
        let known = |name: &str| glyph(name).is_some() || name == "tada";
        assert_eq!(
            link_entities_and_emoji_in_source(
                "lol :clown_face: :+1_medium_skin_tone: :tada: :nope: `:clown_face:`",
                &known,
                &glyph
            ),
            "lol 🤡 👍🏽 ![:tada:](emoji:tada) :nope: `:clown_face:`"
        );
    }

    #[test]
    fn specials_and_markup_around_entities() {
        assert_eq!(
            rewrite("**@here** _~dev_"),
            "**[@here](matty://special/here)** _[~dev](matty://channel/dev)_"
        );
    }

    #[test]
    fn code_links_and_urls_are_left_alone() {
        for text in [
            "`@ana`",
            "```\n@ana ~dev\n```",
            "[@ana](https://example.com)",
            "see https://example.com/~dev/#section ok",
            "mail ana@example.com",
        ] {
            assert_eq!(rewrite(text), text);
        }
    }

    #[test]
    fn labels_are_escaped() {
        assert_eq!(
            rewrite("@first_last"),
            r"[@first\_last](matty://user/first_last)"
        );
    }

    #[test]
    fn escaped_text_is_kept() {
        assert_eq!(rewrite(r"\@ana \#tag"), r"\@ana \#tag");
    }

    #[test]
    fn tables_and_lists() {
        assert_eq!(
            rewrite("- @ana\n\n| a |\n| - |\n| ~dev |"),
            "- [@ana](matty://user/ana)\n\n| a |\n| - |\n| [~dev](matty://channel/dev) |"
        );
    }
}
