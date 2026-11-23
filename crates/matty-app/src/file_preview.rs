//! A text or code file's contents in a dialog (#1002), as the official
//! client previews them, instead of handing a source file to the OS.

use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};

use crate::style::{self, TypeStyleExt as _};
use crate::t;
use crate::ui::{
    button::{Button, ButtonVariant},
    dialog::{DialogHandle, dialog_description, dialog_title},
    markdown::markdown,
    theme::UiTheme,
};

/// Larger files open with the system instead.
pub const MAX_BYTES: u64 = 512 * 1024;
/// The preview stops here; the file itself keeps the rest.
const MAX_LINES: usize = 2000;

/// The fence language for a file previewed as text, or `None` when the
/// file is not text Matty previews.
pub fn language(path: &std::path::Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "txt" | "log" | "csv" | "tsv" | "ini" | "conf" | "cfg" | "env" | "diff" | "patch" => "",
        "md" | "markdown" => "markdown",
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "cpp",
        "cs" => "cs",
        "rb" => "ruby",
        "php" => "php",
        "sh" | "bash" | "zsh" => "bash",
        "sql" => "sql",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" | "svg" | "plist" => "xml",
        "html" | "htm" => "html",
        "css" | "scss" => "css",
        "lua" => "lua",
        "dart" => "dart",
        _ => return None,
    })
}

/// The Markdown that shows `text` as one fenced block, with a fence longer
/// than any run of backticks in it, and whether it was cut short.
fn fenced(text: &str, language: &str) -> (String, bool) {
    let lines: Vec<&str> = text.lines().collect();
    let cut = lines.len() > MAX_LINES;
    let body = lines[..lines.len().min(MAX_LINES)].join("\n");
    let longest = body.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    (format!("{fence}{language}\n{body}\n{fence}"), cut)
}

pub struct FilePreviewView {
    name: SharedString,
    path: std::path::PathBuf,
    source: SharedString,
    cut: bool,
    handle: DialogHandle,
    /// Opens on the next render, which has the window.
    pending: bool,
}

impl Default for FilePreviewView {
    fn default() -> Self {
        Self::new()
    }
}

impl FilePreviewView {
    pub fn new() -> Self {
        Self {
            name: SharedString::default(),
            path: std::path::PathBuf::new(),
            source: SharedString::default(),
            cut: false,
            handle: DialogHandle::new(false),
            pending: false,
        }
    }

    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    /// Shows `text`, read from `path`, on the next frame.
    pub fn show(
        &mut self,
        path: std::path::PathBuf,
        text: &str,
        language: &str,
        cx: &mut Context<Self>,
    ) {
        let (source, cut) = fenced(text, language);
        self.name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned().into())
            .unwrap_or_default();
        self.path = path;
        self.source = source.into();
        self.cut = cut;
        self.pending = true;
        cx.notify();
    }

    /// Opens the dialog when [`Self::show`] asked for it.
    pub fn open_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.pending) {
            self.handle.open(window, cx);
        }
    }
}

impl Render for FilePreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        let path = self.path.clone();
        div()
            .flex()
            .flex_col()
            .gap(theme.space(3.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.))
                    .child(dialog_title("file-preview.title", cx).child(self.name.clone()))
                    .child(
                        dialog_description("file-preview.description", cx).child(if self.cut {
                            t!("file-preview-cut", lines = MAX_LINES)
                        } else {
                            t!("file-preview-detail")
                        }),
                    ),
            )
            .child(
                div()
                    .id("file-preview.body")
                    .max_h(theme.space(120.))
                    .overflow_y_scroll()
                    .type_style(style::UI, &theme)
                    .child(markdown("file-preview.markdown", self.source.clone(), cx)),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(theme.space(2.))
                    .child(
                        Button::new("file-preview.reveal")
                            .variant(ButtonVariant::Outline)
                            .label(t!("file-preview-reveal"))
                            .on_click(move |_, _, cx| cx.reveal_path(&path)),
                    )
                    .child(
                        Button::new("file-preview.close")
                            .variant(ButtonVariant::Default)
                            .label(t!("file-preview-close"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle.close(window, cx);
                            })),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_files_have_a_language_and_others_none() {
        assert_eq!(language(std::path::Path::new("main.RS")), Some("rust"));
        assert_eq!(language(std::path::Path::new("notes.txt")), Some(""));
        assert_eq!(language(std::path::Path::new("photo.png")), None);
        assert_eq!(language(std::path::Path::new("Makefile")), None);
    }

    #[test]
    fn the_fence_outgrows_backticks_inside() {
        let (source, cut) = fenced("let a = ```b```;", "rust");
        assert!(source.starts_with("````rust\n"));
        assert!(source.ends_with("\n````"));
        assert!(!cut);
        let long = "x\n".repeat(MAX_LINES + 5);
        assert!(fenced(&long, "").1);
    }
}
