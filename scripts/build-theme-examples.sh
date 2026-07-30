#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_root="$repo_root/tmp/stbvl/examples"

# Keep this list intentionally small: these presets cover light, dark, neutral,
# and more colorful examples without producing an unwieldy gallery.
color_themes=(default slate forest sand midnight blackandwhite)

mapfile -t themes < <(
  cargo run --quiet --manifest-path "$repo_root/Cargo.toml" -- list-themes
)

build_variant() {
  local site_dir="$1"
  local site_name="$2"
  local theme="$3"
  local color_theme="$4"
  local output_name="$5"
  local output_dir="$output_root/$site_name/$output_name"
  local build_args=(
    --theme "$theme"
    --out "$output_dir"
    --no-cache
    --fast-images
    --precompress false
  )

  if [[ -n "$color_theme" ]]; then
    build_args+=(--color-theme "$color_theme")
  fi

  printf 'Building %s with theme=%s%s -> %s\n' \
    "$site_name" "$theme" \
    "${color_theme:+ color=$color_theme}" "$output_dir"
  cargo run --quiet --manifest-path "$repo_root/Cargo.toml" -- \
    --source-dir "$site_dir" --no-writeback build "${build_args[@]}"
}

for site_dir in "$repo_root"/examples/*; do
  [[ -f "$site_dir/stbl.yaml" ]] || continue
  site_name="$(basename "$site_dir")"

  for theme in "${themes[@]}"; do
    if [[ "$theme" == "liberty" ]]; then
      build_variant "$site_dir" "$site_name" "$theme" "" "${theme}-built-in"
      continue
    fi

    for color_theme in "${color_themes[@]}"; do
      build_variant "$site_dir" "$site_name" "$theme" "$color_theme" "${theme}-${color_theme}"
    done
  done
done
