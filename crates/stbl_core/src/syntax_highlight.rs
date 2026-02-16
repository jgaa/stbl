use std::cell::RefCell;

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use syntect_assets::assets::HighlightingAssets;

thread_local! {
    static ASSETS: RefCell<HighlightingAssets> = RefCell::new(HighlightingAssets::from_binary());
}

fn find_syntax<'a>(syntax_set: &'a SyntaxSet, language: &str) -> Option<&'a SyntaxReference> {
    let token = language.trim().to_lowercase();
    if token.is_empty() {
        return None;
    }
    let token = map_language_alias(&token);
    if let Some(syntax) = syntax_set.find_syntax_by_token(token) {
        return Some(syntax);
    }
    if let Some(ext) = extension_from_token(token) {
        return syntax_set.find_syntax_by_extension(ext);
    }
    None
}

fn map_language_alias(token: &str) -> &str {
    match token {
        "c++" | "cpp" | "cxx" | "cc" => "cpp",
        "sh" | "shell" => "bash",
        _ => token,
    }
}

fn extension_from_token(token: &str) -> Option<&str> {
    if token.chars().any(|ch| ch.is_whitespace()) {
        return None;
    }
    if let Some(stripped) = token.strip_prefix('.') {
        if !stripped.is_empty() {
            return Some(stripped);
        }
    }
    if let Some((_, ext)) = token.rsplit_once('.') {
        if !ext.is_empty() {
            return Some(ext);
        }
    }
    None
}

fn find_theme_name<'a, I>(theme: &str, themes: I) -> Option<String>
where
    I: Iterator<Item = &'a str>,
{
    let trimmed = theme.trim();
    if trimmed.is_empty() {
        return None;
    }
    let needle = trimmed.to_lowercase();
    themes
        .filter_map(|name: &str| {
            if name == trimmed || name.to_lowercase() == needle {
                Some(name.to_string())
            } else {
                None
            }
        })
        .next()
}

pub fn highlight_code_html_classed(code: &str, language: &str, theme: &str) -> Option<String> {
    ASSETS.with(|cell| {
        let assets = cell.borrow();
        let syntax_set = assets.get_syntax_set().ok()?;
        let syntax = find_syntax(syntax_set, language)?;
        let theme_name = find_theme_name(theme, assets.themes())?;
        let _theme = assets.get_theme(&theme_name);

        let mut generator =
            ClassedHTMLGenerator::new_with_class_style(syntax, syntax_set, ClassStyle::Spaced);
        for line in LinesWithEndings::from(code) {
            let _ = generator.parse_html_for_line_which_includes_newline(line);
        }
        Some(generator.finalize())
    })
}

#[cfg(test)]
mod tests {
    use super::highlight_code_html_classed;

    #[test]
    fn highlights_rust_code() {
        let html = highlight_code_html_classed("fn main() {}\n", "rs", "GitHub")
            .expect("expected highlight");
        assert!(html.contains("fn"));
    }

    #[test]
    fn alias_maps_to_cpp() {
        let html = highlight_code_html_classed("int x = 1;\n", "c++", "GitHub")
            .expect("expected highlight");
        assert!(html.contains("int"));
    }

    #[test]
    fn missing_theme_returns_none() {
        assert!(highlight_code_html_classed("fn main() {}", "rs", "NoSuchTheme").is_none());
    }

    #[test]
    fn missing_language_returns_none() {
        assert!(
            highlight_code_html_classed("fn main() {}", "no-such-lang-xyz", "GitHub").is_none()
        );
    }
}
