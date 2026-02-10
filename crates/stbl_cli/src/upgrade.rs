use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UpgradeOutput {
    pub yaml: String,
    pub warnings: Vec<String>,
}

pub fn upgrade_site(source_dir: &Path, force: bool) -> Result<UpgradeOutput> {
    let legacy_path = source_dir.join("stbl.conf");
    if !legacy_path.exists() {
        bail!("stbl.conf not found");
    }
    let yaml_path = source_dir.join("stbl.yaml");
    if yaml_path.exists() && !force {
        bail!("stbl.yaml already exists (use --force to overwrite)");
    }

    let raw = fs::read_to_string(&legacy_path)
        .with_context(|| format!("failed to read {}", legacy_path.display()))?;
    let tokens = tokenize(&strip_comments(&raw))?;
    let mut pos = 0;
    let root = parse_entries(&tokens, &mut pos, false)?;
    let (config, warnings) = convert_legacy(&root, source_dir)?;

    let mut yaml = serde_yaml::to_string(&config).context("failed to serialize yaml")?;
    yaml = compact_numeric_lists(&yaml);
    if !yaml.ends_with('\n') {
        yaml.push('\n');
    }

    fs::write(&yaml_path, &yaml)
        .with_context(|| format!("failed to write {}", yaml_path.display()))?;

    remove_legacy_scaled_media(source_dir)?;

    Ok(UpgradeOutput { yaml, warnings })
}

fn compact_numeric_lists(input: &str) -> String {
    let mut out = String::new();
    let mut lines = input.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_end();
        let key = trimmed.trim_start();
        if key == "widths:" || key == "heights:" {
            let indent_len = line.len().saturating_sub(line.trim_start().len());
            let indent = " ".repeat(indent_len);
            let mut values = Vec::new();
            while let Some(next) = lines.peek() {
                let next_trim = next.trim_start();
                if !next_trim.starts_with("- ") {
                    break;
                }
                if next.len() < indent_len + 2 {
                    break;
                }
                let raw = next_trim.trim_start_matches("- ").trim();
                if !raw.is_empty() && raw.chars().all(|ch| ch.is_ascii_digit()) {
                    values.push(raw.to_string());
                } else {
                    values.clear();
                    break;
                }
                lines.next();
            }
            if values.is_empty() {
                out.push_str(trimmed);
                out.push('\n');
            } else {
                out.push_str(&format!(
                    "{indent}{} [{}]\n",
                    key,
                    values.join(", ")
                ));
            }
            continue;
        }
        out.push_str(trimmed);
        out.push('\n');
    }
    out
}

fn remove_legacy_scaled_media(source_dir: &Path) -> Result<()> {
    let images_dir = source_dir.join("images");
    if images_dir.exists() {
        for entry in fs::read_dir(&images_dir)
            .with_context(|| format!("failed to read {}", images_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("_scale") {
                fs::remove_dir_all(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            }
        }
    }

    let video_dir = source_dir.join("video");
    if video_dir.exists() {
        for entry in fs::read_dir(&video_dir)
            .with_context(|| format!("failed to read {}", video_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("_scale") || name == "_poster_" {
                fs::remove_dir_all(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct UpgradeConfig {
    site: SiteOut,
    theme: ThemeOut,
    assets: AssetsOut,
    media: MediaOut,
    footer: FooterOut,
    blog: BlogOut,
    banner: Option<BannerOut>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    menu: Vec<MenuItemOut>,
    people: Option<PeopleOut>,
    system: Option<SystemOut>,
    publish: Option<PublishOut>,
    rss: Option<RssOut>,
    comments: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct SiteOut {
    id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tagline: Option<String>,
    base_url: String,
    language: String,
    url_style: String,
}

#[derive(Debug, Clone, Serialize)]
struct ThemeOut {
    max_body_width: String,
    breakpoints: ThemeBreakpointsOut,
}

#[derive(Debug, Clone, Serialize)]
struct ThemeBreakpointsOut {
    desktop_min: String,
    wide_min: String,
}

#[derive(Debug, Clone, Serialize)]
struct AssetsOut {
    cache_busting: bool,
}

#[derive(Debug, Clone, Serialize)]
struct MediaOut {
    images: ImageOut,
    video: VideoOut,
}

#[derive(Debug, Clone, Serialize)]
struct ImageOut {
    widths: Vec<u32>,
    quality: u8,
}

#[derive(Debug, Clone, Serialize)]
struct VideoOut {
    heights: Vec<u32>,
    poster_time: u32,
}

#[derive(Debug, Clone, Serialize)]
struct FooterOut {
    show_stbl: bool,
}

#[derive(Debug, Clone, Serialize)]
struct BannerOut {
    widths: Vec<u32>,
    quality: u32,
    align: i32,
}

#[derive(Debug, Clone, Serialize)]
struct MenuItemOut {
    title: String,
    href: String,
}

#[derive(Debug, Clone, Serialize)]
struct PeopleOut {
    default: String,
    entries: BTreeMap<String, PersonOut>,
}

#[derive(Debug, Clone, Serialize)]
struct PersonOut {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    links: Vec<PersonLinkOut>,
}

#[derive(Debug, Clone, Serialize)]
struct PersonLinkOut {
    id: String,
    name: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SystemOut {
    date: Option<SystemDateOut>,
}

#[derive(Debug, Clone, Serialize)]
struct SystemDateOut {
    format: String,
    roundup_seconds: u32,
}

#[derive(Debug, Clone, Serialize)]
struct PublishOut {
    command: String,
}

#[derive(Debug, Clone, Serialize)]
struct RssOut {
    enabled: bool,
    max_items: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl_channel: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
struct BlogOut {
    #[serde(rename = "abstract")]
    abstract_cfg: BlogAbstractOut,
    pagination: BlogPaginationOut,
    series: BlogSeriesOut,
}

#[derive(Debug, Clone, Serialize)]
struct BlogAbstractOut {
    enabled: bool,
    max_chars: usize,
}

#[derive(Debug, Clone, Serialize)]
struct BlogPaginationOut {
    enabled: bool,
    page_size: usize,
}

#[derive(Debug, Clone, Serialize)]
struct BlogSeriesOut {
    latest_parts: usize,
}

fn convert_legacy(root: &Node, source_dir: &Path) -> Result<(UpgradeConfig, Vec<String>)> {
    let mut warnings = Vec::new();
    let title = match value_of(root, "name") {
        Some(value) => value,
        None => bail!("missing required legacy field: name"),
    };
    let base_url = match value_of(root, "url") {
        Some(value) => value,
        None => bail!("missing required legacy field: url"),
    };
    let language = value_of(root, "language").unwrap_or_else(|| "en".to_string());
    let tagline = value_of(root, "abstract");
    let id = derive_site_id(source_dir, &title);

    let max_articles = value_of(root, "max-articles-on-frontpage")
        .and_then(|value| parse_usize(&value, "max-articles-on-frontpage", &mut warnings));

    let banner = block_of(root, "banner")
        .map(|block| parse_banner(block, &mut warnings))
        .transpose()?;
    let menu = block_of(root, "menu")
        .map(|block| parse_menu(block, &mut warnings))
        .unwrap_or_default();
    let people = if let Some(block) = block_of(root, "people") {
        parse_people(block, &mut warnings)?
    } else {
        None
    };
    let system = if let Some(block) = block_of(root, "system") {
        parse_system(block, &mut warnings)?
    } else {
        None
    };
    let publish = block_of(root, "publish")
        .and_then(|block| value_of(block, "command"))
        .map(|command| PublishOut { command });
    let rss = block_of(root, "rss")
        .map(|block| parse_rss(block, &mut warnings))
        .transpose()?;

    let comments = block_of(root, "comments").and_then(|block| parse_comments(block, &mut warnings));

    if block_of(root, "chroma").is_some() {
        warnings.push("legacy chroma section ignored".to_string());
    }
    if block_of(root, "plyr").is_some() {
        warnings.push("legacy plyr section ignored".to_string());
    }

    let blog_pagination = match max_articles {
        Some(page_size) => BlogPaginationOut {
            enabled: true,
            page_size,
        },
        None => BlogPaginationOut {
            enabled: false,
            page_size: 10,
        },
    };

    let config = UpgradeConfig {
        site: SiteOut {
            id,
            title,
            tagline,
            base_url,
            language,
            url_style: "html".to_string(),
        },
        theme: ThemeOut {
            max_body_width: "72rem".to_string(),
            breakpoints: ThemeBreakpointsOut {
                desktop_min: "768px".to_string(),
                wide_min: "1400px".to_string(),
            },
        },
        assets: AssetsOut {
            cache_busting: false,
        },
        media: MediaOut {
            images: ImageOut {
                widths: default_image_widths(),
                quality: 90,
            },
            video: VideoOut {
                heights: default_video_heights(),
                poster_time: 1,
            },
        },
        footer: FooterOut { show_stbl: true },
        blog: BlogOut {
            abstract_cfg: BlogAbstractOut {
                enabled: true,
                max_chars: 200,
            },
            pagination: blog_pagination,
            series: BlogSeriesOut { latest_parts: 3 },
        },
        banner,
        menu,
        people,
        system,
        publish,
        rss,
        comments,
    };

    Ok((config, warnings))
}

fn parse_banner(block: &Node, warnings: &mut Vec<String>) -> Result<BannerOut> {
    let widths = value_of(block, "widths")
        .map(parse_widths)
        .transpose()?
        .unwrap_or_else(default_image_widths);
    let quality = value_of(block, "quality")
        .and_then(|value| parse_u32(&value, "banner.quality", warnings))
        .unwrap_or(90);
    let align = value_of(block, "align")
        .and_then(|value| parse_i32(&value, "banner.align", warnings))
        .unwrap_or(0);
    Ok(BannerOut {
        widths,
        quality,
        align,
    })
}

fn parse_menu(block: &Node, warnings: &mut Vec<String>) -> Vec<MenuItemOut> {
    let mut items = Vec::new();
    for entry in &block.entries {
        let href = match entry.value.clone() {
            Some(value) => value,
            None => {
                let slug = slugify(&entry.key);
                if slug.is_empty() {
                    warnings.push(format!(
                        "menu item '{}' is missing href and was skipped",
                        entry.key
                    ));
                    continue;
                }
                format!("{slug}.html")
            }
        };
        items.push(MenuItemOut {
            title: entry.key.clone(),
            href,
        });
    }
    items
}

fn parse_people(block: &Node, warnings: &mut Vec<String>) -> Result<Option<PeopleOut>> {
    let mut entries = BTreeMap::new();
    let mut default = None;

    for entry in &block.entries {
        if entry.key == "default" {
            if let Some(value) = entry.value.clone() {
                default = Some(value);
            }
            continue;
        }
        let Some(person_block) = entry.block.as_ref() else {
            continue;
        };
        let Some(name) = value_of(person_block, "name") else {
            warnings.push(format!(
                "people entry '{}' is missing name and was skipped",
                entry.key
            ));
            continue;
        };
        let mut links = Vec::new();
        if let Some(email_block) = block_of(person_block, "e-mail") {
            let link_name = value_of(email_block, "name").unwrap_or_else(|| "Contact".to_string());
            if let Some(url) = value_of(email_block, "url") {
                let icon = value_of(email_block, "icon");
                links.push(PersonLinkOut {
                    id: "e-mail".to_string(),
                    name: link_name,
                    url,
                    icon,
                });
            } else {
                warnings.push(format!(
                    "people entry '{}' has e-mail block without url",
                    entry.key
                ));
            }
        }
        entries.insert(
            entry.key.clone(),
            PersonOut {
                name,
                email: None,
                links,
            },
        );
    }

    if entries.is_empty() {
        return Ok(None);
    }

    let default = match default {
        Some(value) if entries.contains_key(&value) => value,
        Some(value) => {
            warnings.push(format!(
                "people.default '{}' not found; using first entry",
                value
            ));
            entries
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "default".to_string())
        }
        None => entries
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "default".to_string()),
    };

    Ok(Some(PeopleOut { default, entries }))
}

fn parse_system(block: &Node, warnings: &mut Vec<String>) -> Result<Option<SystemOut>> {
    let date_block = block_of(block, "date");
    if date_block.is_none() {
        return Ok(None);
    }
    let date_block = date_block.unwrap();
    let format = value_of(date_block, "format");
    let roundup = value_of(date_block, "roundup")
        .and_then(|value| parse_u32(&value, "system.date.roundup", warnings))
        .unwrap_or(0);
    let date = format.map(|format| SystemDateOut {
        format,
        roundup_seconds: roundup,
    });
    Ok(Some(SystemOut { date }))
}

fn parse_rss(block: &Node, warnings: &mut Vec<String>) -> Result<RssOut> {
    let enabled = value_of(block, "enabled")
        .map(|value| parse_bool(&value))
        .unwrap_or(true);
    let max_items = match value_of(block, "max-articles")
        .and_then(|value| parse_usize(&value, "rss.max-articles", warnings))
    {
        Some(value) => value,
        None if enabled => return Err(anyhow!("rss.enabled requires max-articles")),
        None => 10,
    };

    let ttl_channel = value_of(block, "ttl")
        .and_then(|value| parse_u32(&value, "rss.ttl", warnings))
        .filter(|value| *value > 0);

    Ok(RssOut {
        enabled,
        max_items,
        ttl_channel,
        ttl_days: None,
    })
}

fn parse_simple_map(block: &Node) -> serde_yaml::Value {
    let mut map = serde_yaml::Mapping::new();
    for entry in &block.entries {
        if let Some(value) = entry.value.clone() {
            map.insert(
                serde_yaml::Value::String(entry.key.clone()),
                parse_scalar_value(&value),
            );
        } else if let Some(child) = entry.block.as_ref() {
            let nested = parse_simple_map(child);
            map.insert(serde_yaml::Value::String(entry.key.clone()), nested);
        }
    }
    serde_yaml::Value::Mapping(map)
}

fn parse_comments(block: &Node, warnings: &mut Vec<String>) -> Option<serde_yaml::Value> {
    let value = parse_simple_map(block);
    match &value {
        serde_yaml::Value::Mapping(map) if map.is_empty() => None,
        serde_yaml::Value::Mapping(map) => {
            if let Some(default) = map.get(&serde_yaml::Value::String("default".to_string())) {
                if default.as_str().map(|val| val.trim().is_empty()).unwrap_or(true) {
                    warnings.push("comments.default is empty and was ignored".to_string());
                }
            }
            Some(value)
        }
        _ => Some(value),
    }
}

fn parse_scalar_value(value: &str) -> serde_yaml::Value {
    if let Some(bool_val) = parse_bool_strict(value) {
        return serde_yaml::Value::Bool(bool_val);
    }
    if let Ok(int_val) = value.parse::<i64>() {
        return serde_yaml::Value::Number(int_val.into());
    }
    serde_yaml::Value::String(value.to_string())
}

fn parse_bool(value: &str) -> bool {
    parse_bool_strict(value).unwrap_or(false)
}

fn parse_bool_strict(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

fn parse_widths(value: String) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    for item in value.split(',') {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        let width = trimmed
            .parse::<u32>()
            .with_context(|| format!("invalid width '{trimmed}'"))?;
        out.push(width);
    }
    if out.is_empty() {
        bail!("banner.widths is empty");
    }
    Ok(out)
}

fn parse_u32(value: &str, label: &str, warnings: &mut Vec<String>) -> Option<u32> {
    match value.trim().parse::<u32>() {
        Ok(value) => Some(value),
        Err(_) => {
            warnings.push(format!("{label} value '{value}' is invalid"));
            None
        }
    }
}

fn parse_i32(value: &str, label: &str, warnings: &mut Vec<String>) -> Option<i32> {
    match value.trim().parse::<i32>() {
        Ok(value) => Some(value),
        Err(_) => {
            warnings.push(format!("{label} value '{value}' is invalid"));
            None
        }
    }
}

fn parse_usize(value: &str, label: &str, warnings: &mut Vec<String>) -> Option<usize> {
    match value.trim().parse::<usize>() {
        Ok(value) => Some(value),
        Err(_) => {
            warnings.push(format!("{label} value '{value}' is invalid"));
            None
        }
    }
}

fn derive_site_id(source_dir: &Path, title: &str) -> String {
    let fallback = source_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(title);
    let slug = slugify(fallback);
    if slug.is_empty() {
        "site".to_string()
    } else {
        slug
    }
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.starts_with('-') {
        out.remove(0);
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn default_image_widths() -> Vec<u32> {
    vec![
        94, 128, 248, 360, 480, 640, 720, 950, 1280, 1440, 1680, 1920, 2560,
    ]
}

fn default_video_heights() -> Vec<u32> {
    vec![360, 480, 720, 1080]
}

#[derive(Debug, Clone)]
struct Node {
    entries: Vec<Entry>,
}

#[derive(Debug, Clone)]
struct Entry {
    key: String,
    value: Option<String>,
    block: Option<Node>,
}

fn value_of(node: &Node, key: &str) -> Option<String> {
    node.entries
        .iter()
        .find(|entry| entry.key == key)
        .and_then(|entry| entry.value.clone())
}

fn block_of<'a>(node: &'a Node, key: &str) -> Option<&'a Node> {
    node.entries
        .iter()
        .find(|entry| entry.key == key)
        .and_then(|entry| entry.block.as_ref())
}

#[derive(Debug, Clone)]
enum Token {
    Ident(String),
    Str(String),
    LBrace,
    RBrace,
    Newline,
}

fn strip_comments(input: &str) -> String {
    let mut out = String::new();
    for line in input.lines() {
        let mut in_string = false;
        let mut result = String::new();
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '"' {
                in_string = !in_string;
                result.push(ch);
                continue;
            }
            if !in_string && ch == ';' {
                break;
            }
            result.push(ch);
        }
        if !result.trim().is_empty() {
            out.push_str(&result);
            out.push('\n');
        }
    }
    out
}

fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\n' {
            tokens.push(Token::Newline);
            continue;
        }
        if ch.is_whitespace() {
            continue;
        }
        match ch {
            '{' => tokens.push(Token::LBrace),
            '}' => tokens.push(Token::RBrace),
            '"' => {
                let mut buf = String::new();
                while let Some(next) = chars.next() {
                    if next == '"' {
                        break;
                    }
                    if next == '\\' {
                        if let Some(escaped) = chars.next() {
                            buf.push(escaped);
                        }
                    } else {
                        buf.push(next);
                    }
                }
                tokens.push(Token::Str(buf));
            }
            _ => {
                let mut buf = String::new();
                buf.push(ch);
                while let Some(next) = chars.peek() {
                    if next.is_whitespace() || *next == '{' || *next == '}' {
                        break;
                    }
                    buf.push(*next);
                    chars.next();
                }
                tokens.push(Token::Ident(buf));
            }
        }
    }
    Ok(tokens)
}

fn parse_entries(tokens: &[Token], pos: &mut usize, stop_on_rbrace: bool) -> Result<Node> {
    fn skip_newlines(tokens: &[Token], pos: &mut usize) {
        while matches!(tokens.get(*pos), Some(Token::Newline)) {
            *pos += 1;
        }
    }

    let mut entries = Vec::new();
    while *pos < tokens.len() {
        skip_newlines(tokens, pos);
        if stop_on_rbrace {
            if matches!(tokens[*pos], Token::RBrace) {
                *pos += 1;
                break;
            }
        }
        let key = match tokens.get(*pos) {
            Some(Token::Ident(text)) => text.clone(),
            Some(Token::Str(text)) => text.clone(),
            Some(Token::RBrace) if stop_on_rbrace => break,
            Some(token) => bail!("unexpected token in key: {token:?}"),
            None => break,
        };
        *pos += 1;

        let mut value = None;
        let mut block = None;
        match tokens.get(*pos) {
            Some(Token::LBrace) => {
                *pos += 1;
                block = Some(parse_entries(tokens, pos, true)?);
            }
            Some(Token::Ident(text)) => {
                value = Some(text.clone());
                *pos += 1;
            }
            Some(Token::Str(text)) => {
                value = Some(text.clone());
                *pos += 1;
            }
            Some(Token::Newline) | None => {}
            Some(Token::RBrace) => {
                if stop_on_rbrace {
                    continue;
                }
            }
        }

        skip_newlines(tokens, pos);
        if matches!(tokens.get(*pos), Some(Token::LBrace)) {
            *pos += 1;
            block = Some(parse_entries(tokens, pos, true)?);
        }

        entries.push(Entry { key, value, block });
    }
    Ok(Node { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_basic_blocks() {
        let input = "name \"Demo\"\nbanner { widths \"94, 128\" }";
        let tokens = tokenize(&strip_comments(input)).expect("tokenize");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn parse_menu_defaults_href_without_consuming_next_item() {
        let input = "menu {\n  Contact\n  About\n}\n";
        let tokens = tokenize(&strip_comments(input)).expect("tokenize");
        let mut pos = 0;
        let root = parse_entries(&tokens, &mut pos, false).expect("parse entries");
        let block = block_of(&root, "menu").expect("menu block");
        let mut warnings = Vec::new();
        let items = parse_menu(block, &mut warnings);
        assert!(warnings.is_empty());
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Contact");
        assert_eq!(items[0].href, "contact.html");
        assert_eq!(items[1].title, "About");
        assert_eq!(items[1].href, "about.html");
    }

    #[test]
    fn comments_are_copied_and_chroma_plyr_ignored() {
        let input = r#"
name "Demo"
url "https://example.com/"
comments {
  default disqus
  disqus {
    src "https://example.disqus.com/embed.js"
    template "disqus.html"
  }
}
chroma {
  enabled auto
}
plyr {
  js "https://cdn.plyr.io/3.7.8/plyr.js"
}
"#;
        let tokens = tokenize(&strip_comments(input)).expect("tokenize");
        let mut pos = 0;
        let root = parse_entries(&tokens, &mut pos, false).expect("parse entries");
        let temp_dir = std::env::temp_dir();
        let (config, warnings) = convert_legacy(&root, &temp_dir).expect("convert");
        let comments = config.comments.expect("comments");
        let map = comments.as_mapping().expect("mapping");
        assert!(map.contains_key(&serde_yaml::Value::String("default".to_string())));
        assert!(map.contains_key(&serde_yaml::Value::String("disqus".to_string())));
        assert!(warnings.iter().any(|warning| warning.contains("chroma section ignored")));
        assert!(warnings.iter().any(|warning| warning.contains("plyr section ignored")));
    }

    #[test]
    fn compact_numeric_lists_formats_widths_and_heights() {
        let input = "\
media:\n  images:\n    widths:\n    - 94\n    - 128\n    quality: 90\n  video:\n    heights:\n    - 360\n    - 720\n    poster_time: 1\n";
        let output = compact_numeric_lists(input);
        assert!(output.contains("    widths: [94, 128]\n"));
        assert!(output.contains("    heights: [360, 720]\n"));
        assert!(output.contains("    quality: 90\n"));
        assert!(output.contains("    poster_time: 1\n"));
    }
}
