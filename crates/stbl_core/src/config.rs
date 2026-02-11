use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::model::{
    AssetsConfig, BannerConfig, FooterConfig, ImageConfig, ImageFormatMode, MediaConfig, MenuAlign,
    MenuItem, NavItem, PeopleConfig, PersonEntry, PublishConfig, RssConfig, SecurityConfig,
    SeoConfig, SiteConfig, SiteMeta, SvgSecurityConfig, SvgSecurityMode, SyntaxConfig, SystemConfig,
    ThemeBreakpoints, ThemeColorOverrides, ThemeColorScheme, ThemeConfig, ThemeHeaderConfig,
    ThemeHeaderLayout, ThemeNavOverrides, ThemeWideBackgroundOverrides, UrlStyle, VideoConfig,
    WideBackgroundStyle,
};

#[derive(Debug, Deserialize)]
struct SiteConfigRaw {
    site: SiteMetaRaw,
    banner: Option<BannerConfigRaw>,
    #[serde(default)]
    menu: Vec<MenuItem>,
    theme: Option<ThemeConfigRaw>,
    syntax: Option<SyntaxConfigRaw>,
    assets: Option<AssetsConfigRaw>,
    security: Option<SecurityConfigRaw>,
    media: Option<MediaConfigRaw>,
    footer: Option<FooterConfigRaw>,
    people: Option<PeopleConfigRaw>,
    blog: Option<BlogConfigRaw>,
    system: Option<SystemConfig>,
    publish: Option<PublishConfig>,
    rss: Option<RssConfigRaw>,
    seo: Option<SeoConfig>,
    comments: Option<serde_yaml::Value>,
    chroma: Option<serde_yaml::Value>,
    plyr: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
struct SiteMetaRaw {
    id: Option<String>,
    title: Option<String>,
    tagline: Option<String>,
    logo: Option<String>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    copyright: Option<String>,
    base_url: Option<String>,
    language: Option<String>,
    timezone: Option<String>,
    url_style: Option<UrlStyle>,
    macros: Option<MacrosConfigRaw>,
    nav: Option<Vec<NavItemRaw>>,
}

#[derive(Debug, Deserialize)]
struct FooterConfigRaw {
    show_stbl: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct MacrosConfigRaw {
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SyntaxConfigRaw {
    highlight: Option<bool>,
    theme: Option<String>,
    line_numbers: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AssetsConfigRaw {
    cache_busting: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SecurityConfigRaw {
    svg: Option<SvgSecurityConfigRaw>,
}

#[derive(Debug, Deserialize)]
struct SvgSecurityConfigRaw {
    mode: Option<SvgSecurityMode>,
}

#[derive(Debug, Deserialize)]
struct BannerConfigRaw {
    widths: Option<Vec<u32>>,
    quality: Option<u32>,
    align: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct ThemeConfigRaw {
    variant: Option<String>,
    max_body_width: Option<String>,
    breakpoints: Option<ThemeBreakpointsRaw>,
    colors: Option<ThemeColorsRaw>,
    nav: Option<ThemeNavRaw>,
    header: Option<ThemeHeaderRaw>,
    wide_background: Option<ThemeWideBackgroundRaw>,
    color_scheme: Option<ThemeColorScheme>,
}

#[derive(Debug, Deserialize)]
struct ThemeBreakpointsRaw {
    desktop_min: Option<String>,
    wide_min: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ThemeColorsRaw {
    bg: Option<String>,
    fg: Option<String>,
    heading: Option<String>,
    accent: Option<String>,
    link: Option<String>,
    muted: Option<String>,
    surface: Option<String>,
    border: Option<String>,
    link_hover: Option<String>,
    code_bg: Option<String>,
    code_fg: Option<String>,
    quote_bg: Option<String>,
    quote_border: Option<String>,
    wide_bg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ThemeNavRaw {
    bg: Option<String>,
    fg: Option<String>,
    border: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ThemeHeaderRaw {
    layout: Option<ThemeHeaderLayout>,
    menu_align: Option<MenuAlign>,
    title_size: Option<String>,
    tagline_size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ThemeWideBackgroundRaw {
    color: Option<String>,
    image: Option<String>,
    style: Option<String>,
    position: Option<String>,
    opacity: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct MediaConfigRaw {
    images: Option<ImageConfigRaw>,
    video: Option<VideoConfigRaw>,
}

#[derive(Debug, Deserialize)]
struct ImageConfigRaw {
    widths: Option<Vec<u32>>,
    quality: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct VideoConfigRaw {
    heights: Option<Vec<u32>>,
    poster_time: Option<PosterTimeRaw>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PosterTimeRaw {
    Int(u32),
    String(String),
}

#[derive(Debug, Deserialize)]
struct NavItemRaw {
    label: Option<String>,
    href: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PeopleConfigRaw {
    default: Option<String>,
    entries: Option<BTreeMap<String, PersonEntry>>,
}

#[derive(Debug, Deserialize)]
struct BlogConfigRaw {
    #[serde(rename = "abstract")]
    abstract_cfg: Option<BlogAbstractConfigRaw>,
    page_size: Option<usize>,
    pagination: Option<BlogPaginationConfigRaw>,
    series: Option<BlogSeriesConfigRaw>,
}

#[derive(Debug, Deserialize)]
struct BlogAbstractConfigRaw {
    enabled: Option<bool>,
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct BlogPaginationConfigRaw {
    enabled: Option<bool>,
    page_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct BlogSeriesConfigRaw {
    latest_parts: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RssConfigRaw {
    enabled: Option<bool>,
    max_items: Option<usize>,
    ttl_channel: Option<i64>,
    ttl_days: Option<i64>,
}

pub fn load_site_config(path: &Path) -> Result<SiteConfig> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    let parsed: SiteConfigRaw = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse YAML config {}", path.display()))?;

    let site = SiteMeta {
        id: required_string(parsed.site.id, "site.id")?,
        title: required_string(parsed.site.title, "site.title")?,
        tagline: optional_non_empty(parsed.site.tagline, "site.tagline")?
            .or_else(|| parsed.site.abstract_text),
        logo: optional_non_empty(parsed.site.logo, "site.logo")?,
        copyright: parsed.site.copyright,
        base_url: required_string(parsed.site.base_url, "site.base_url")?,
        language: required_string(parsed.site.language, "site.language")?,
        timezone: parsed.site.timezone,
        url_style: parsed.site.url_style.unwrap_or_default(),
        macros: crate::model::MacrosConfig {
            enabled: parsed.site.macros.and_then(|macros| macros.enabled).unwrap_or(true),
        },
    };

    let nav = match parsed.site.nav {
        Some(items) => parse_nav_items(items)?,
        None if !parsed.menu.is_empty() => parsed
            .menu
            .iter()
            .map(|item| NavItem {
                label: item.title.clone(),
                href: item.href.clone(),
            })
            .collect(),
        None => Vec::new(),
    };

    let people = match parsed.people {
        None => None,
        Some(people_raw) => {
            let entries = match people_raw.entries {
                Some(entries) => entries,
                None => bail!("missing required field: people.entries"),
            };
            if entries.is_empty() {
                bail!("people.entries must not be empty");
            }
            let default = match people_raw.default {
                Some(value) if !value.trim().is_empty() => {
                    if !entries.contains_key(&value) {
                        bail!("people.default '{}' is missing from people.entries", value);
                    }
                    value
                }
                _ => entries
                    .keys()
                    .next()
                    .cloned()
                    .expect("entries should not be empty"),
            };
            Some(PeopleConfig { default, entries })
        }
    };

    let blog = match parsed.blog {
        None => None,
        Some(blog_raw) => {
            let (abstract_enabled, abstract_max_chars) = match blog_raw.abstract_cfg {
                Some(abstract_raw) => (
                    abstract_raw.enabled.unwrap_or(true),
                    abstract_raw.max_chars.unwrap_or(200),
                ),
                None => (true, 200),
            };
            if abstract_enabled && abstract_max_chars == 0 {
                bail!("blog.abstract.max_chars must be > 0 when abstract is enabled");
            }
            let (enabled, page_size) = match blog_raw.pagination {
                Some(pagination_raw) => (
                    pagination_raw.enabled.unwrap_or(false),
                    pagination_raw.page_size.unwrap_or(10),
                ),
                None => (
                    blog_raw.page_size.is_some(),
                    blog_raw.page_size.unwrap_or(10),
                ),
            };
            if enabled && page_size == 0 {
                bail!("blog.pagination.page_size must be > 0 when pagination is enabled");
            }
            Some(crate::model::BlogConfig {
                abstract_cfg: crate::model::BlogAbstractConfig {
                    enabled: abstract_enabled,
                    max_chars: abstract_max_chars,
                },
                pagination: crate::model::BlogPaginationConfig { enabled, page_size },
                series: crate::model::BlogSeriesConfig {
                    latest_parts: blog_raw
                        .series
                        .and_then(|series| series.latest_parts)
                        .unwrap_or(3),
                },
            })
        }
    };

    let rss = match parsed.rss {
        None => None,
        Some(rss_raw) => {
            let enabled = rss_raw.enabled.unwrap_or(false);
            if enabled && rss_raw.max_items.is_none() {
                bail!("rss.max_items required when rss.enabled is true");
            }
            let ttl_days = match rss_raw.ttl_days {
                None => None,
                Some(value) => {
                    if value <= 0 {
                        bail!("rss.ttl_days must be > 0");
                    }
                    Some(u32::try_from(value).context("rss.ttl_days out of range")?)
                }
            };
            let ttl_channel = match rss_raw.ttl_channel {
                None => None,
                Some(value) => {
                    if value < 0 {
                        bail!("rss.ttl_channel must be >= 0");
                    }
                    if value == 0 {
                        None
                    } else {
                        Some(u32::try_from(value).context("rss.ttl_channel out of range")?)
                    }
                }
            };
            Some(RssConfig {
                enabled,
                max_items: rss_raw.max_items,
                ttl_channel,
                ttl_days,
            })
        }
    };

    let theme_raw = parsed.theme.as_ref();
    let theme = ThemeConfig {
        variant: non_empty_or_default(
            theme_raw.and_then(|theme| theme.variant.clone()),
            "default",
            "theme.variant",
        )?,
        max_body_width: non_empty_or_default(
            theme_raw.and_then(|theme| theme.max_body_width.clone()),
            "72rem",
            "theme.max_body_width",
        )?,
        breakpoints: ThemeBreakpoints {
            desktop_min: non_empty_or_default(
                theme_raw
                    .and_then(|theme| theme.breakpoints.as_ref())
                    .and_then(|breakpoints| breakpoints.desktop_min.clone()),
                "768px",
                "theme.breakpoints.desktop_min",
            )?,
            wide_min: non_empty_or_default(
                theme_raw
                    .and_then(|theme| theme.breakpoints.as_ref())
                    .and_then(|breakpoints| breakpoints.wide_min.clone()),
                "1400px",
                "theme.breakpoints.wide_min",
            )?,
        },
        colors: ThemeColorOverrides {
            bg: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.bg.clone()),
                "theme.colors.bg",
            )?,
            fg: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.fg.clone()),
                "theme.colors.fg",
            )?,
            heading: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.heading.clone()),
                "theme.colors.heading",
            )?,
            accent: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.accent.clone()),
                "theme.colors.accent",
            )?,
            link: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.link.clone()),
                "theme.colors.link",
            )?,
            muted: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.muted.clone()),
                "theme.colors.muted",
            )?,
            surface: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.surface.clone()),
                "theme.colors.surface",
            )?,
            border: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.border.clone()),
                "theme.colors.border",
            )?,
            link_hover: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.link_hover.clone()),
                "theme.colors.link_hover",
            )?,
            code_bg: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.code_bg.clone()),
                "theme.colors.code_bg",
            )?,
            code_fg: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.code_fg.clone()),
                "theme.colors.code_fg",
            )?,
            quote_bg: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.quote_bg.clone()),
                "theme.colors.quote_bg",
            )?,
            quote_border: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.quote_border.clone()),
                "theme.colors.quote_border",
            )?,
            wide_bg: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.colors.as_ref())
                    .and_then(|colors| colors.wide_bg.clone()),
                "theme.colors.wide_bg",
            )?,
        },
        nav: ThemeNavOverrides {
            bg: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.nav.as_ref())
                    .and_then(|nav| nav.bg.clone()),
                "theme.nav.bg",
            )?,
            fg: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.nav.as_ref())
                    .and_then(|nav| nav.fg.clone()),
                "theme.nav.fg",
            )?,
            border: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.nav.as_ref())
                    .and_then(|nav| nav.border.clone()),
                "theme.nav.border",
            )?,
        },
        header: ThemeHeaderConfig {
            layout: theme_raw
                .and_then(|theme| theme.header.as_ref())
                .and_then(|header| header.layout)
                .unwrap_or_default(),
            menu_align: theme_raw
                .and_then(|theme| theme.header.as_ref())
                .and_then(|header| header.menu_align)
                .unwrap_or_default(),
            title_size: non_empty_or_default(
                theme_raw
                    .and_then(|theme| theme.header.as_ref())
                    .and_then(|header| header.title_size.clone()),
                "1.3rem",
                "theme.header.title_size",
            )?,
            tagline_size: non_empty_or_default(
                theme_raw
                    .and_then(|theme| theme.header.as_ref())
                    .and_then(|header| header.tagline_size.clone()),
                "1rem",
                "theme.header.tagline_size",
            )?,
        },
        wide_background: ThemeWideBackgroundOverrides {
            color: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.wide_background.as_ref())
                    .and_then(|wide| wide.color.clone()),
                "theme.wide_background.color",
            )?,
            image: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.wide_background.as_ref())
                    .and_then(|wide| wide.image.clone()),
                "theme.wide_background.image",
            )?,
            style: theme_raw
                .and_then(|theme| theme.wide_background.as_ref())
                .and_then(|wide| wide.style.as_ref())
                .map(|value| parse_wide_background_style(value))
                .transpose()?,
            position: optional_non_empty(
                theme_raw
                    .and_then(|theme| theme.wide_background.as_ref())
                    .and_then(|wide| wide.position.clone()),
                "theme.wide_background.position",
            )?,
            opacity: theme_raw
                .and_then(|theme| theme.wide_background.as_ref())
                .and_then(|wide| wide.opacity)
                .map(validate_opacity)
                .transpose()?,
        },
        color_scheme: theme_raw.and_then(|theme| theme.color_scheme.clone()),
    };

    let syntax = {
        let highlight = parsed
            .syntax
            .as_ref()
            .and_then(|syntax| syntax.highlight)
            .unwrap_or(true);
        let theme = non_empty_or_fallback(
            parsed
                .syntax
                .as_ref()
                .and_then(|syntax| syntax.theme.clone()),
            "GitHub",
        );
        let line_numbers = parsed
            .syntax
            .as_ref()
            .and_then(|syntax| syntax.line_numbers)
            .unwrap_or(true);
        SyntaxConfig {
            highlight,
            theme,
            line_numbers,
        }
    };

    let security = SecurityConfig {
        svg: SvgSecurityConfig {
            mode: parsed
                .security
                .as_ref()
                .and_then(|security| security.svg.as_ref())
                .and_then(|svg| svg.mode)
                .unwrap_or(SvgSecurityMode::Warn),
        },
    };

    let media = MediaConfig {
        images: ImageConfig {
            widths: parsed
                .media
                .as_ref()
                .and_then(|media| media.images.as_ref())
                .and_then(|images| images.widths.clone())
                .unwrap_or_else(default_image_widths),
            quality: parsed
                .media
                .as_ref()
                .and_then(|media| media.images.as_ref())
                .and_then(|images| images.quality)
                .unwrap_or(90),
            format_mode: ImageFormatMode::Normal,
        },
        video: VideoConfig {
            heights: parsed
                .media
                .as_ref()
                .and_then(|media| media.video.as_ref())
                .and_then(|video| video.heights.clone())
                .unwrap_or_else(default_video_heights),
            poster_time_sec: parsed
                .media
                .as_ref()
                .and_then(|media| media.video.as_ref())
                .and_then(|video| video.poster_time.as_ref())
                .map(parse_poster_time)
                .transpose()?
                .unwrap_or(1),
        },
    };

    Ok(SiteConfig {
        site,
        banner: parsed.banner.map(|banner| BannerConfig {
            widths: banner.widths.unwrap_or_else(default_image_widths),
            quality: banner.quality.unwrap_or(90),
            align: banner.align.unwrap_or(0),
        }),
        menu: parsed.menu,
        nav,
        theme,
        syntax,
        assets: AssetsConfig {
            cache_busting: parsed
                .assets
                .and_then(|assets| assets.cache_busting)
                .unwrap_or(false),
        },
        security,
        media,
        footer: FooterConfig {
            show_stbl: parsed
                .footer
                .and_then(|footer| footer.show_stbl)
                .unwrap_or(true),
        },
        people,
        blog,
        system: parsed.system,
        publish: parsed.publish,
        rss,
        seo: parsed.seo,
        comments: parsed.comments,
        chroma: parsed.chroma,
        plyr: parsed.plyr,
    })
}

fn required_string(value: Option<String>, field: &str) -> Result<String> {
    match value {
        Some(text) if !text.trim().is_empty() => Ok(text),
        _ => bail!("missing required field: {}", field),
    }
}

fn parse_nav_items(items: Vec<NavItemRaw>) -> Result<Vec<NavItem>> {
    let mut out = Vec::with_capacity(items.len());
    for (idx, item) in items.into_iter().enumerate() {
        let label = required_string(item.label, &format!("site.nav[{idx}].label"))?;
        let href = required_string(item.href, &format!("site.nav[{idx}].href"))?;
        out.push(NavItem { label, href });
    }
    Ok(out)
}

fn non_empty_or_default(value: Option<String>, default: &str, field: &str) -> Result<String> {
    match value {
        Some(text) => {
            if text.trim().is_empty() {
                bail!("{field} must not be empty");
            }
            Ok(text)
        }
        None => Ok(default.to_string()),
    }
}

fn optional_non_empty(value: Option<String>, field: &str) -> Result<Option<String>> {
    match value {
        Some(text) => {
            if text.trim().is_empty() {
                bail!("{field} must not be empty");
            }
            Ok(Some(text))
        }
        None => Ok(None),
    }
}

fn non_empty_or_fallback(value: Option<String>, default: &str) -> String {
    match value {
        Some(text) if !text.trim().is_empty() => text,
        _ => default.to_string(),
    }
}

fn parse_wide_background_style(value: &str) -> Result<WideBackgroundStyle> {
    match value.trim() {
        "cover" => Ok(WideBackgroundStyle::Cover),
        "tile" => Ok(WideBackgroundStyle::Tile),
        _ => bail!("theme.wide_background.style must be 'cover' or 'tile'"),
    }
}

fn validate_opacity(value: f32) -> Result<f32> {
    if !value.is_finite() {
        bail!("theme.wide_background.opacity must be a finite number");
    }
    if !(0.0..=1.0).contains(&value) {
        bail!("theme.wide_background.opacity must be between 0.0 and 1.0");
    }
    Ok(value)
}

fn default_image_widths() -> Vec<u32> {
    vec![
        94, 128, 248, 360, 480, 640, 720, 950, 1280, 1440, 1680, 1920, 2560,
    ]
}

fn default_video_heights() -> Vec<u32> {
    vec![360, 480, 720, 1080]
}

fn parse_poster_time(value: &PosterTimeRaw) -> Result<u32> {
    match value {
        PosterTimeRaw::Int(value) => Ok(*value),
        PosterTimeRaw::String(text) => {
            let trimmed = text.trim();
            if let Some(stripped) = trimmed.strip_suffix('s') {
                return stripped
                    .trim()
                    .parse::<u32>()
                    .context("media.video.poster_time must be a positive integer");
            }
            trimmed
                .parse::<u32>()
                .context("media.video.poster_time must be a positive integer")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::load_site_config;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn write_temp(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("stbl-config-{}.yaml", Uuid::new_v4()));
        fs::write(&path, contents).expect("write temp config");
        path
    }

    #[test]
    fn valid_minimal_config_parses() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
        );
        let config = load_site_config(&path).expect("config should load");
        assert_eq!(config.site.id, "demo");
    }

    #[test]
    fn theme_defaults_apply_when_missing() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
        );
        let config = load_site_config(&path).expect("config should load");
        assert_eq!(config.theme.variant, "default");
        assert_eq!(config.theme.max_body_width, "72rem");
        assert_eq!(config.theme.breakpoints.desktop_min, "768px");
        assert_eq!(config.theme.breakpoints.wide_min, "1400px");
        assert!(config.theme.colors.bg.is_none());
        assert!(config.theme.nav.bg.is_none());
        assert!(config.theme.wide_background.color.is_none());
        assert_eq!(
            config.theme.header.layout,
            crate::model::ThemeHeaderLayout::Stacked
        );
        assert_eq!(config.theme.header.menu_align, crate::model::MenuAlign::Right);
        assert_eq!(config.theme.header.title_size, "1.3rem");
        assert_eq!(config.theme.header.tagline_size, "1rem");
    }

    #[test]
    fn syntax_defaults_apply_when_missing() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
        );
        let config = load_site_config(&path).expect("config should load");
        assert!(config.syntax.highlight);
        assert_eq!(config.syntax.theme, "GitHub");
        assert!(config.syntax.line_numbers);
    }

    #[test]
    fn syntax_theme_empty_defaults_to_github() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nsyntax:\n  theme: \"\"\n",
        );
        let config = load_site_config(&path).expect("config should load");
        assert_eq!(config.syntax.theme, "GitHub");
    }

    #[test]
    fn security_svg_defaults_to_warn() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
        );
        let config = load_site_config(&path).expect("config should load");
        assert_eq!(config.security.svg.mode, crate::model::SvgSecurityMode::Warn);
    }

    #[test]
    fn security_svg_mode_parses() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nsecurity:\n  svg:\n    mode: \"sanitize\"\n",
        );
        let config = load_site_config(&path).expect("config should load");
        assert_eq!(
            config.security.svg.mode,
            crate::model::SvgSecurityMode::Sanitize
        );
    }

    #[test]
    fn banner_and_media_quality_default_when_omitted() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n\
banner: {}\n\
media:\n  images:\n    widths: [200]\n",
        );
        let config = load_site_config(&path).expect("config should load");
        let banner = config.banner.expect("banner");
        assert_eq!(banner.quality, 90);
        assert_eq!(banner.align, 0);
        assert_eq!(banner.widths, super::default_image_widths());
        assert_eq!(config.media.images.quality, 90);
    }

    #[test]
    fn missing_required_field_fails() {
        let path = write_temp(
            "site:\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
        );
        let err = load_site_config(&path).expect_err("expected error");
        assert!(err.to_string().contains("site.id"));
    }

    #[test]
    fn rss_enabled_without_required_fields_fails() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nrss:\n  enabled: true\n",
        );
        let err = load_site_config(&path).expect_err("expected error");
        assert!(err.to_string().contains("rss.max_items"));
    }

    #[test]
    fn invalid_ttl_days_fails() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nrss:\n  enabled: true\n  max_items: 10\n  ttl_days: 0\n",
        );
        let err = load_site_config(&path).expect_err("expected error");
        assert!(err.to_string().contains("rss.ttl_days"));
    }

    #[test]
    fn rss_ttl_channel_zero_is_ignored() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nrss:\n  enabled: true\n  max_items: 10\n  ttl_channel: 0\n",
        );
        let config = load_site_config(&path).expect("config should load");
        let rss = config.rss.expect("rss should be present");
        assert!(rss.ttl_channel.is_none());
    }

    #[test]
    fn people_default_falls_back_to_first_entry() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\npeople:\n  entries:\n    alice:\n      name: \"Alice\"\n    bob:\n      name: \"Bob\"\n",
        );
        let config = load_site_config(&path).expect("config should load");
        let people = config.people.expect("people should be present");
        assert_eq!(people.default, "alice");
    }

    #[test]
    fn blog_defaults_apply() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nblog: {}\n",
        );
        let config = load_site_config(&path).expect("config should load");
        let blog = config.blog.expect("blog should be present");
        assert!(blog.abstract_cfg.enabled);
        assert_eq!(blog.abstract_cfg.max_chars, 200);
        assert!(!blog.pagination.enabled);
        assert_eq!(blog.pagination.page_size, 10);
        assert_eq!(blog.series.latest_parts, 3);
    }

    #[test]
    fn blog_pagination_requires_page_size_when_enabled() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nblog:\n  pagination:\n    enabled: true\n    page_size: 0\n",
        );
        let err = load_site_config(&path).expect_err("expected error");
        assert!(
            err.to_string()
                .contains("blog.pagination.page_size must be > 0")
        );
    }

    #[test]
    fn blog_page_size_enables_pagination_for_legacy_config() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nblog:\n  page_size: 5\n",
        );
        let config = load_site_config(&path).expect("config should load");
        let blog = config.blog.expect("blog should be present");
        assert!(blog.pagination.enabled);
        assert_eq!(blog.pagination.page_size, 5);
    }

    #[test]
    fn blog_abstract_requires_max_chars_when_enabled() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nblog:\n  abstract:\n    enabled: true\n    max_chars: 0\n",
        );
        let err = load_site_config(&path).expect_err("expected error");
        assert!(
            err.to_string()
                .contains("blog.abstract.max_chars must be > 0")
        );
    }

    #[test]
    fn blog_abstract_can_be_disabled() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nblog:\n  abstract:\n    enabled: false\n    max_chars: 10\n",
        );
        let config = load_site_config(&path).expect("config should load");
        let blog = config.blog.expect("blog should be present");
        assert!(!blog.abstract_cfg.enabled);
        assert_eq!(blog.abstract_cfg.max_chars, 10);
    }

    #[test]
    fn theme_header_layout_and_align_parse() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\ntheme:\n  header:\n    layout: inline\n    menu_align: center\n    title_size: 1.5rem\n    tagline_size: 1.1rem\n",
        );
        let config = load_site_config(&path).expect("config should load");
        assert_eq!(
            config.theme.header.layout,
            crate::model::ThemeHeaderLayout::Inline
        );
        assert_eq!(config.theme.header.menu_align, crate::model::MenuAlign::Center);
        assert_eq!(config.theme.header.title_size, "1.5rem");
        assert_eq!(config.theme.header.tagline_size, "1.1rem");
    }
}
