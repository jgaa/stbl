#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_root="$repo_root/tmp/stbvl/examples"
# Set this when deploying the gallery below a reverse-proxy path, for example:
# STBL_EXAMPLES_BASE_URL=https://nextapp.org/sites/a/b/c
gallery_base_url="${STBL_EXAMPLES_BASE_URL:-http://127.0.0.1:8000}"

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
  local site_base_url="${gallery_base_url%/}/$site_name/$output_name/"
  local build_args=(
    --theme "$theme"
    --base-url "$site_base_url"
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

mkdir -p "$output_root"
{
  printf '%s\n' '<!doctype html>' '<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>stbl theme examples</title><style>body{font:16px system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem}li{margin:.4rem 0}</style></head><body>' '<h1>stbl theme examples</h1>' '<p>Generated theme and color variants.</p>'
  for site_dir in "$output_root"/*; do
    [[ -d "$site_dir" ]] || continue
    site_name="$(basename "$site_dir")"
    printf '<h2>%s</h2><ul>\n' "$site_name"
    for variant_dir in "$site_dir"/*; do
      [[ -d "$variant_dir" ]] || continue
      variant_name="$(basename "$variant_dir")"
      printf '<li><a href="%s/%s/">%s</a></li>\n' "$site_name" "$variant_name" "$variant_name"
    done
    printf '%s\n' '</ul>'
  done
  printf '%s\n' '</body></html>'
} > "$output_root/index.html"
printf 'Wrote gallery index: %s\n' "$output_root/index.html"
