use eframe::egui;
use eframe::egui::text::{LayoutJob, TextFormat};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Plain,
    C,
    Cpp,
    Rust,
    Zig,
    Assembly,
}

impl Language {
    pub fn name(self) -> &'static str {
        match self {
            Self::Plain => "Plain text",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::Rust => "Rust",
            Self::Zig => "Zig",
            Self::Assembly => "Assembly",
        }
    }
}

pub fn language_for_path(path: &Path) -> Language {
    let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "c" | "h" => Language::C,
        "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => Language::Cpp,
        "rs" => Language::Rust,
        "zig" => Language::Zig,
        "s" | "asm" => Language::Assembly,
        _ => Language::Plain,
    }
}

pub fn highlighted_job(ui: &egui::Ui, source: &str, language: Language, wrap_width: f32, font_size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;

    if source.is_empty() {
        job.append("", 0.0, format(ui, font_size, ColorRole::Normal));
        return job;
    }

    if source.len() > 1_500_000 || language == Language::Plain {
        job.append(source, 0.0, format(ui, font_size, ColorRole::Normal));
        return job;
    }

    let mut start = 0;
    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            highlight_line(ui, &mut job, &source[start..=idx], language, font_size);
            start = idx + 1;
        }
    }
    if start < source.len() {
        highlight_line(ui, &mut job, &source[start..], language, font_size);
    }

    job
}

fn highlight_line(ui: &egui::Ui, job: &mut LayoutJob, line: &str, language: Language, font_size: f32) {
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];

        if is_line_comment_start(rest, language) {
            job.append(rest, 0.0, format(ui, font_size, ColorRole::Comment));
            return;
        }

        if rest.starts_with("/*") && matches!(language, Language::C | Language::Cpp | Language::Rust | Language::Zig) {
            let end = rest.find("*/").map(|idx| idx + 2).unwrap_or(rest.len());
            job.append(&rest[..end], 0.0, format(ui, font_size, ColorRole::Comment));
            i += end;
            continue;
        }

        let ch = rest.chars().next().unwrap_or_default();

        if ch == '"' || ch == '\'' {
            let end = scan_string(rest, ch);
            job.append(&rest[..end], 0.0, format(ui, font_size, ColorRole::String));
            i += end;
            continue;
        }

        if ch.is_ascii_digit() {
            let end = scan_number(rest);
            job.append(&rest[..end], 0.0, format(ui, font_size, ColorRole::Number));
            i += end;
            continue;
        }

        if is_ident_start(ch) {
            let end = scan_ident(rest);
            let ident = &rest[..end];
            let role = if is_keyword(language, ident) {
                ColorRole::Keyword
            } else if is_type_name(ident) {
                ColorRole::Type
            } else {
                ColorRole::Normal
            };
            job.append(ident, 0.0, format(ui, font_size, role));
            i += end;
            continue;
        }

        let width = ch.len_utf8();
        job.append(&rest[..width], 0.0, format(ui, font_size, ColorRole::Normal));
        i += width;
    }
}

fn is_line_comment_start(rest: &str, language: Language) -> bool {
    match language {
        Language::C | Language::Cpp | Language::Rust | Language::Zig => rest.starts_with("//"),
        Language::Assembly => rest.starts_with(';') || rest.starts_with('#'),
        Language::Plain => false,
    }
}

fn scan_string(rest: &str, quote: char) -> usize {
    let mut escaped = false;
    for (idx, ch) in rest.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return idx + ch.len_utf8();
        }
        if ch == '\n' {
            return idx;
        }
    }
    rest.len()
}

fn scan_number(rest: &str) -> usize {
    for (idx, ch) in rest.char_indices() {
        if !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | 'x' | 'X' | 'b' | 'B')) {
            return idx;
        }
    }
    rest.len()
}

fn scan_ident(rest: &str) -> usize {
    for (idx, ch) in rest.char_indices() {
        if idx == 0 {
            continue;
        }
        if !is_ident_continue(ch) {
            return idx;
        }
    }
    rest.len()
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_type_name(ident: &str) -> bool {
    matches!(
        ident,
        "char"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "void"
            | "bool"
            | "size_t"
            | "ssize_t"
            | "uint8_t"
            | "uint16_t"
            | "uint32_t"
            | "uint64_t"
            | "int8_t"
            | "int16_t"
            | "int32_t"
            | "int64_t"
            | "usize"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "f32"
            | "f64"
    )
}

fn is_keyword(language: Language, ident: &str) -> bool {
    keyword_list(language).contains(&ident)
}

fn keyword_list(language: Language) -> &'static [&'static str] {
    match language {
        Language::C => &[
            "auto",
            "break",
            "case",
            "const",
            "continue",
            "default",
            "do",
            "else",
            "enum",
            "extern",
            "for",
            "goto",
            "if",
            "inline",
            "register",
            "restrict",
            "return",
            "sizeof",
            "static",
            "struct",
            "switch",
            "typedef",
            "union",
            "volatile",
            "while",
            "_Atomic",
            "_Generic",
            "_Noreturn",
            "_Static_assert",
            "_Thread_local",
        ],
        Language::Cpp => &[
            "alignas",
            "alignof",
            "and",
            "asm",
            "auto",
            "bitand",
            "bitor",
            "break",
            "case",
            "catch",
            "class",
            "concept",
            "const",
            "consteval",
            "constexpr",
            "constinit",
            "const_cast",
            "continue",
            "decltype",
            "default",
            "delete",
            "do",
            "dynamic_cast",
            "else",
            "enum",
            "explicit",
            "export",
            "extern",
            "false",
            "for",
            "friend",
            "goto",
            "if",
            "inline",
            "mutable",
            "namespace",
            "new",
            "noexcept",
            "nullptr",
            "operator",
            "private",
            "protected",
            "public",
            "requires",
            "return",
            "sizeof",
            "static",
            "static_assert",
            "static_cast",
            "struct",
            "switch",
            "template",
            "this",
            "thread_local",
            "throw",
            "true",
            "try",
            "typedef",
            "typeid",
            "typename",
            "union",
            "using",
            "virtual",
            "volatile",
            "while",
        ],
        Language::Rust => &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false",
            "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
            "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
        ],
        Language::Zig => &[
            "addrspace",
            "align",
            "allowzero",
            "and",
            "anyframe",
            "anytype",
            "asm",
            "async",
            "await",
            "break",
            "callconv",
            "catch",
            "comptime",
            "const",
            "continue",
            "defer",
            "else",
            "enum",
            "errdefer",
            "error",
            "export",
            "extern",
            "fn",
            "for",
            "if",
            "inline",
            "linksection",
            "noalias",
            "noinline",
            "nosuspend",
            "opaque",
            "or",
            "orelse",
            "packed",
            "pub",
            "resume",
            "return",
            "struct",
            "suspend",
            "switch",
            "test",
            "threadlocal",
            "try",
            "union",
            "unreachable",
            "usingnamespace",
            "var",
            "volatile",
            "while",
        ],
        Language::Assembly => &["section", "global", "extern", "bits", "default", "segment"],
        Language::Plain => &[],
    }
}

#[derive(Debug, Clone, Copy)]
enum ColorRole {
    Normal,
    Keyword,
    Type,
    String,
    Number,
    Comment,
}

fn format(ui: &egui::Ui, font_size: f32, role: ColorRole) -> TextFormat {
    let visuals = ui.visuals();
    let color = match role {
        ColorRole::Normal => visuals.text_color(),
        ColorRole::Keyword => egui::Color32::from_rgb(220, 170, 90),
        ColorRole::Type => egui::Color32::from_rgb(100, 190, 140),
        ColorRole::String => egui::Color32::from_rgb(190, 130, 210),
        ColorRole::Number => egui::Color32::from_rgb(220, 110, 110),
        ColorRole::Comment => egui::Color32::from_rgb(110, 150, 155),
    };

    TextFormat { font_id: egui::FontId::monospace(font_size), color, ..Default::default() }
}
