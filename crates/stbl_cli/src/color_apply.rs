use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};
use stbl_core::model::{
    ThemeColorScheme, ThemeColorSchemeBase, ThemeColorSchemeSource, ThemeWideBackgroundOverrides,
    WideBackgroundStyle,
};

use crate::color_derive;
use crate::color_presets;

pub fn apply_preset_to_yaml(
    doc: &mut Value,
    name: &str,
    preset: &color_presets::ColorPreset,
) -> Result<()> {
    let root = ensure_mapping(doc, "stbl.yaml root")?;
    let theme_value = root
        .entry(Value::String("theme".to_string()))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let theme = ensure_mapping(theme_value, "theme")?;

    let colors_value =
        serde_yaml::to_value(&preset.colors).context("failed to serialize colors")?;
    theme.insert(Value::String("colors".to_string()), colors_value);

    let nav_value = serde_yaml::to_value(&preset.nav).context("failed to serialize nav colors")?;
    theme.insert(Value::String("nav".to_string()), nav_value);

    if let Some(wide) = preset.wide_background.as_ref() {
        let wide_value = wide_background_to_value(wide)?;
        theme.insert(Value::String("wide_background".to_string()), wide_value);
    }

    let mut scheme = Mapping::new();
    scheme.insert(
        Value::String("name".to_string()),
        Value::String(name.to_string()),
    );
    scheme.insert(
        Value::String("source".to_string()),
        Value::String("preset".to_string()),
    );
    scheme.insert(
        Value::String("mode".to_string()),
        Value::String("auto".to_string()),
    );
    theme.insert(
        Value::String("color_scheme".to_string()),
        Value::Mapping(scheme),
    );

    Ok(())
}

pub fn apply_derived_to_yaml(doc: &mut Value, derived: &color_derive::DerivedScheme) -> Result<()> {
    let root = ensure_mapping(doc, "stbl.yaml root")?;
    let theme_value = root
        .entry(Value::String("theme".to_string()))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let theme = ensure_mapping(theme_value, "theme")?;

    let colors_value =
        serde_yaml::to_value(&derived.colors).context("failed to serialize colors")?;
    theme.insert(Value::String("colors".to_string()), colors_value);

    let nav_value = serde_yaml::to_value(&derived.nav).context("failed to serialize nav colors")?;
    theme.insert(Value::String("nav".to_string()), nav_value);

    let wide_value = wide_background_to_value(&derived.wide_background)?;
    theme.insert(Value::String("wide_background".to_string()), wide_value);

    let scheme = ThemeColorScheme {
        name: None,
        mode: Some(derived.mode),
        source: Some(ThemeColorSchemeSource::Derived),
        base: Some(ThemeColorSchemeBase {
            bg: Some(derived.base.bg.clone()),
            fg: Some(derived.base.fg.clone()),
            accent: Some(derived.base.accent.clone()),
            link: derived.base.link.clone(),
            heading: derived.base.heading.clone(),
        }),
    };
    let scheme_value = serde_yaml::to_value(&scheme).context("failed to serialize color_scheme")?;
    theme.insert(Value::String("color_scheme".to_string()), scheme_value);

    Ok(())
}

fn ensure_mapping<'a>(value: &'a mut Value, label: &str) -> Result<&'a mut Mapping> {
    match value {
        Value::Mapping(map) => Ok(map),
        _ => bail!("{label} must be a mapping"),
    }
}

fn wide_background_to_value(wide: &ThemeWideBackgroundOverrides) -> Result<Value> {
    let mut map = Mapping::new();
    if let Some(value) = wide.color.as_ref() {
        map.insert(
            Value::String("color".to_string()),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = wide.image.as_ref() {
        map.insert(
            Value::String("image".to_string()),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = wide.style {
        let style = match value {
            WideBackgroundStyle::Cover => "cover",
            WideBackgroundStyle::Tile => "tile",
        };
        map.insert(
            Value::String("style".to_string()),
            Value::String(style.to_string()),
        );
    }
    if let Some(value) = wide.position.as_ref() {
        map.insert(
            Value::String("position".to_string()),
            Value::String(value.clone()),
        );
    }
    if let Some(value) = wide.opacity {
        map.insert(
            Value::String("opacity".to_string()),
            Value::from(value as f64),
        );
    }
    Ok(Value::Mapping(map))
}
