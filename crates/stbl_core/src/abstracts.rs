use crate::render::render_markdown_to_html;

const ELLIPSIS: char = '…';

pub fn derive_abstract_from_markdown(markdown: &str, max_chars: usize) -> Option<String> {
    if max_chars == 0 {
        return None;
    }
    let chunk = first_non_empty_chunk(markdown)?;
    let html = render_markdown_to_html(chunk);
    let text = strip_html_tags(&html);
    let collapsed = collapse_whitespace(&text);
    if collapsed.is_empty() {
        return None;
    }
    Some(truncate_with_ellipsis(&collapsed, max_chars))
}

fn first_non_empty_chunk(markdown: &str) -> Option<&str> {
    for chunk in markdown.split("\n\n") {
        let trimmed = chunk.trim();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
    let mut count = 0usize;
    let mut last_space = None;
    let mut cutoff = text.len();
    for (idx, ch) in text.char_indices() {
        if count >= max_chars {
            cutoff = idx;
            break;
        }
        if ch.is_whitespace() {
            last_space = Some(idx);
        }
        count += 1;
    }
    if count < max_chars {
        return text.to_string();
    }
    let cut = last_space.unwrap_or(cutoff);
    let mut truncated = text[..cut].trim_end().to_string();
    if truncated.is_empty() {
        truncated = text[..cutoff].to_string();
    }
    truncated.push(ELLIPSIS);
    truncated
}

#[cfg(test)]
mod tests {
    use super::derive_abstract_from_markdown;

    #[test]
    fn uses_first_paragraph_only() {
        let md = "First paragraph.\n\nSecond paragraph.";
        let abstract_text = derive_abstract_from_markdown(md, 200).expect("abstract");
        assert_eq!(abstract_text, "First paragraph.");
    }

    #[test]
    fn strips_markdown_to_plain_text() {
        let md = "Hello **world** and [link](https://example.com).";
        let abstract_text = derive_abstract_from_markdown(md, 200).expect("abstract");
        assert_eq!(abstract_text, "Hello world and link.");
    }

    #[test]
    fn truncates_with_ellipsis_at_word_boundary() {
        let md = "This is a long paragraph for truncation.";
        let abstract_text = derive_abstract_from_markdown(md, 10).expect("abstract");
        assert_eq!(abstract_text, "This is a…");
    }

    #[test]
    fn truncation_keeps_valid_utf8() {
        let md = "Snowman ☃ and emoji 👍 are here.";
        let abstract_text = derive_abstract_from_markdown(md, 12).expect("abstract");
        assert!(abstract_text.is_char_boundary(abstract_text.len()));
        assert!(abstract_text.ends_with('…'));
    }
}
