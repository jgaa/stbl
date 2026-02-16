use std::path::Path;

pub fn deduce_title_from_source_path(source_path: &str) -> Option<String> {
    let path = Path::new(source_path);
    let stem = path.file_stem().and_then(|value| value.to_str())?.trim();
    if stem.is_empty() {
        return None;
    }
    let base = if stem == "index" {
        if crate::url::logical_key_from_source_path(source_path) == "index" {
            return None;
        }
        let parent = path.parent()?;
        parent.file_name().and_then(|value| value.to_str())?.trim()
    } else {
        stem
    };
    if base.is_empty() {
        return None;
    }
    Some(titleize_segment(base))
}

fn titleize_segment(value: &str) -> String {
    let out = value.replace('_', " ");
    let mut chars = out.chars();
    let Some(first) = chars.next() else {
        return out;
    };
    let mut result = String::new();
    for ch in first.to_uppercase() {
        result.push(ch);
    }
    result.push_str(chars.as_str());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduces_title_from_filename() {
        let title = deduce_title_from_source_path("articles/hello_world.md");
        assert_eq!(title.as_deref(), Some("Hello world"));
    }

    #[test]
    fn deduces_title_from_series_index_dir() {
        let title = deduce_title_from_source_path("articles/fun_with_gRPC_and_C++/index.md");
        assert_eq!(title.as_deref(), Some("Fun with gRPC and C++"));
    }

    #[test]
    fn skips_root_index() {
        let title = deduce_title_from_source_path("articles/index.md");
        assert_eq!(title, None);
    }
}
