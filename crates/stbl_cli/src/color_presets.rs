use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use stbl_core::model::{ThemeColorOverrides, ThemeNavOverrides, ThemeWideBackgroundOverrides};

#[derive(Debug, Clone, Deserialize)]
pub struct ColorPreset {
    pub variant: Option<String>,
    #[serde(default)]
    pub colors: ThemeColorOverrides,
    #[serde(default)]
    pub nav: ThemeNavOverrides,
    pub wide_background: Option<ThemeWideBackgroundOverrides>,
}

pub fn load_color_presets() -> Result<BTreeMap<String, ColorPreset>> {
    let yaml = include_str!("../../stbl_embedded_assets/assets/color-presets.yaml");
    let presets: BTreeMap<String, ColorPreset> =
        serde_yaml::from_str(yaml).context("failed to parse embedded color-presets.yaml")?;
    Ok(presets)
}

pub fn load_color_presets_for_root(root: &Path) -> Result<BTreeMap<String, ColorPreset>> {
    let mut presets = load_color_presets()?;
    let overrides_dir = root.join("stbl").join("color-presets");
    if !overrides_dir.is_dir() {
        return Ok(presets);
    }

    let mut paths = fs::read_dir(&overrides_dir)
        .with_context(|| format!("failed to read {}", overrides_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext, "yaml" | "yml"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let parsed_map = serde_yaml::from_str::<BTreeMap<String, ColorPreset>>(&raw);
        if let Ok(map) = parsed_map {
            for (name, preset) in map {
                presets.insert(name, preset);
            }
            continue;
        }

        let preset = serde_yaml::from_str::<ColorPreset>(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("invalid preset file name {}", path.display()))?;
        presets.insert(name.to_string(), preset);
    }
    Ok(presets)
}

pub fn validate_preset(name: &str, preset: &ColorPreset) -> Result<()> {
    fn require(value: &Option<String>, name: &str, field: &str) -> Result<()> {
        if value.as_ref().is_some_and(|v| !v.trim().is_empty()) {
            Ok(())
        } else {
            bail!("preset '{name}' is missing colors.{field}");
        }
    }

    require(&preset.colors.bg, name, "bg")?;
    require(&preset.colors.fg, name, "fg")?;
    require(&preset.colors.heading, name, "heading")?;
    require(&preset.colors.accent, name, "accent")?;
    require(&preset.colors.link, name, "link")?;
    require(&preset.colors.muted, name, "muted")?;
    require(&preset.colors.surface, name, "surface")?;
    require(&preset.colors.border, name, "border")?;
    require(&preset.colors.link_hover, name, "link_hover")?;
    require(&preset.colors.code_bg, name, "code_bg")?;
    require(&preset.colors.code_fg, name, "code_fg")?;
    require(&preset.colors.quote_bg, name, "quote_bg")?;
    require(&preset.colors.quote_border, name, "quote_border")?;
    require(&preset.colors.wide_bg, name, "wide_bg")?;

    require(&preset.nav.bg, name, "nav.bg")?;
    require(&preset.nav.fg, name, "nav.fg")?;
    require(&preset.nav.border, name, "nav.border")?;

    if let Some(wide) = preset.wide_background.as_ref() {
        if wide
            .color
            .as_ref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
        {
            bail!("preset '{name}' is missing wide_background.color");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{load_color_presets, load_color_presets_for_root, validate_preset};

    #[test]
    fn presets_are_complete() {
        let presets = load_color_presets().expect("load presets");
        for (name, preset) in presets {
            validate_preset(&name, &preset).expect("preset should be complete");
        }
    }

    #[test]
    fn local_presets_override_embedded_presets() {
        let temp = TempDir::new().expect("tempdir");
        let presets_dir = temp.path().join("stbl").join("color-presets");
        fs::create_dir_all(&presets_dir).expect("mkdir presets");
        let custom = r##"
stbl:
  variant: stbl
  colors:
    bg: "#101010"
    fg: "#f0f0f0"
    heading: "#ffffff"
    accent: "#33cc99"
    link: "#66ddaa"
    muted: "#999999"
    surface: "#1a1a1a"
    border: "#2a2a2a"
    link_hover: "#88ffcc"
    code_bg: "#050505"
    code_fg: "#f0f0f0"
    quote_bg: "#171717"
    quote_border: "#33cc99"
    wide_bg: "#111111"
  nav:
    bg: "#0a0a0a"
    fg: "#f0f0f0"
    border: "#2a2a2a"
newtheme:
  variant: newtheme
  colors:
    bg: "#ffffff"
    fg: "#111111"
    heading: "#111111"
    accent: "#ff3366"
    link: "#0033cc"
    muted: "#666666"
    surface: "#f5f5f5"
    border: "#dddddd"
    link_hover: "#002299"
    code_bg: "#f1f1f1"
    code_fg: "#111111"
    quote_bg: "#f7f7f7"
    quote_border: "#dddddd"
    wide_bg: "#fafafa"
  nav:
    bg: "#ffffff"
    fg: "#111111"
    border: "#dddddd"
"##;
        fs::write(presets_dir.join("custom.yaml"), custom).expect("write custom presets");

        let presets = load_color_presets_for_root(temp.path()).expect("load presets");
        assert!(presets.contains_key("newtheme"));
        let stbl = presets.get("stbl").expect("stbl preset");
        assert_eq!(stbl.colors.bg.as_deref(), Some("#101010"));
    }
}
