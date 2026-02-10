use std::collections::BTreeMap;

use anyhow::Result;
use serde_yaml::{Mapping, Value};

use crate::model::{Page, Project};

pub trait CommentTemplateProvider {
    fn load_template(&self, template: &str) -> Result<Option<String>>;
}

pub fn render_comments_html(
    project: &Project,
    page: &Page,
    current_href: &str,
    provider: Option<&dyn CommentTemplateProvider>,
) -> Option<String> {
    let comments = project.config.comments.as_ref()?;
    let mapping = comments.as_mapping()?;
    let selection = page
        .header
        .comments
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if matches!(selection, Some(value) if value.eq_ignore_ascii_case("no")) {
        return None;
    }
    let provider_name = selection
        .map(str::to_string)
        .or_else(|| default_provider_name(mapping))?;
    let provider_value = mapping.get(&Value::String(provider_name.clone()))?;
    let provider_map = provider_value.as_mapping()?;
    let template_value = get_string(provider_map, "template")?;
    let template = resolve_template(template_value, provider, project, page, current_href)?;
    let vars = build_comment_vars(provider_map, &provider_name, project, page, current_href);
    Some(render_template_placeholders(&template, &vars))
}

fn default_provider_name(mapping: &Mapping) -> Option<String> {
    let value = mapping.get(&Value::String("default".to_string()))?;
    let raw = value.as_str()?.trim();
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    }
}

fn get_string<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a str> {
    mapping
        .get(&Value::String(key.to_string()))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn resolve_template(
    template: &str,
    provider: Option<&dyn CommentTemplateProvider>,
    _project: &Project,
    _page: &Page,
    _current_href: &str,
) -> Option<String> {
    if looks_like_inline_template(template) {
        return Some(template.to_string());
    }
    let provider = provider?;
    match provider.load_template(template) {
        Ok(Some(contents)) => Some(contents),
        Ok(None) => {
            eprintln!("comments: template not found: {template}");
            None
        }
        Err(err) => {
            eprintln!("comments: failed to load template '{template}': {err}");
            None
        }
    }
}

fn looks_like_inline_template(template: &str) -> bool {
    template.contains('<') || template.contains('\n') || template.contains("{{")
}

fn build_comment_vars(
    provider_map: &Mapping,
    provider_name: &str,
    project: &Project,
    page: &Page,
    current_href: &str,
) -> BTreeMap<String, String> {
    let mut vars = BTreeMap::new();
    if let Some(uuid) = page.header.uuid {
        vars.insert("uuid".to_string(), uuid.to_string());
    }
    if let Some(title) = page.header.title.as_deref() {
        vars.insert("title".to_string(), title.to_string());
    }
    let page_url = format_page_url(project, current_href);
    vars.insert("page_url".to_string(), page_url.clone());
    vars.insert("page-url".to_string(), page_url);

    for (key, value) in provider_map {
        let Some(key_str) = key.as_str() else { continue };
        if key_str == "template" {
            continue;
        }
        let Some(value_str) = yaml_value_to_string(value) else { continue };
        let var_key = format!("{provider_name}-{key_str}");
        vars.insert(var_key, value_str);
    }

    vars
}

fn yaml_value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Bool(v) => Some(v.to_string()),
        Value::Number(v) => Some(v.to_string()),
        Value::String(v) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => None,
    }
}

fn format_page_url(project: &Project, current_href: &str) -> String {
    let base = project.config.site.base_url.trim_end_matches('/');
    let href = current_href.trim_start_matches('/');
    if href.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{href}")
    }
}

fn render_template_placeholders(template: &str, vars: &BTreeMap<String, String>) -> String {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        let (before, after) = rest.split_at(start);
        output.push_str(before);
        let after = &after[2..];
        if let Some(end) = after.find("}}") {
            let key = after[..end].trim();
            if let Some(value) = vars.get(key) {
                output.push_str(value);
            } else {
                output.push_str("{{");
                output.push_str(after[..end].trim_end());
                output.push_str("}}");
            }
            rest = &after[end + 2..];
        } else {
            output.push_str("{{");
            output.push_str(after);
            return output;
        }
    }
    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_site_config;
    use crate::model::{DocId, Page, Project, SiteContent};
    use crate::header::Header;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn comments_disabled_by_header() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\ncomments:\n  default: disqus\n  disqus:\n    template: \"<section>{{disqus-src}}</section>\"\n    src: \"https://example.com/embed.js\"\n",
        );
        let mut page = simple_page("Hello", "articles/hello.md");
        page.header.comments = Some("no".to_string());
        let html = render_comments_html(&project, &page, "hello.html", None);
        assert!(html.is_none());
    }

    #[test]
    fn comments_render_with_inline_template() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\ncomments:\n  default: disqus\n  disqus:\n    template: \"<section>{{disqus-src}}</section>\"\n    src: \"https://example.com/embed.js\"\n",
        );
        let page = simple_page("Hello", "articles/hello.md");
        let html = render_comments_html(&project, &page, "hello.html", None).expect("html");
        assert!(html.contains("https://example.com/embed.js"));
    }

    fn project_with_config(config: &str) -> Project {
        let path = write_temp_config(config);
        let config = load_site_config(&path).expect("config");
        Project {
            root: PathBuf::from("."),
            config,
            content: SiteContent::default(),
            image_alpha: std::collections::BTreeMap::new(),
            image_variants: Default::default(),
            video_variants: Default::default(),
        }
    }

    fn write_temp_config(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("stbl-comments-{}.yaml", Uuid::new_v4()));
        fs::write(&path, contents).expect("write config");
        path
    }

    fn simple_page(title: &str, source_path: &str) -> Page {
        let mut header = Header::default();
        header.title = Some(title.to_string());
        Page {
            id: DocId(blake3::hash(title.as_bytes())),
            source_path: source_path.to_string(),
            header,
            body_markdown: "Body".to_string(),
            banner_name: None,
            media_refs: Vec::new(),
            url_path: "simple".to_string(),
            content_hash: blake3::hash(title.as_bytes()),
        }
    }
}
