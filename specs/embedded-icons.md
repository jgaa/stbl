# Embedded Icon Assets

This project ships with embedded SVG assets for icons. These are bundled into the
binary via `crates/stbl_embedded_assets` and copied into the output directory
when referenced.

## Location

Embedded template assets live under:

`crates/stbl_embedded_assets/assets/templates/default/`

Icons should be placed under:

`crates/stbl_embedded_assets/assets/templates/default/icons/`

Example:

```
crates/stbl_embedded_assets/assets/templates/default/icons/github.svg
```

## How Icons Are Copied

Icons are **not** copied blindly. During the build, the renderer scans generated
HTML/CSS output (and CSS assets) for icon references and only copies the icons
that are actually used. This keeps output size small while still allowing a
large embedded icon set.

The scan recognizes references like:

```
icons/github.svg
artifacts/icons/github.svg
```

and their cache-busted variants via the asset manifest.

## Referencing Icons

Use a normal URL to the icon in your HTML or CSS:

```
<img src="{{rel}}artifacts/icons/github.svg" alt="GitHub">
```

or in CSS:

```
background-image: url("{{rel}}artifacts/icons/github.svg");
```

When cache busting is enabled, the `artifacts/...` URL is automatically resolved
via the asset manifest.

## Adding Simple Icons

We recommend Simple Icons (SVG) for brand logos. Download the SVGs you need and
place them in the `icons/` folder above. Only referenced icons will be copied to
the output.

## Notes

- Only SVGs under `icons/` (and legacy `feather/`) are considered for selective
  copying.
- If an icon is no longer referenced, it will be skipped during the build and
  removed from the output if previously copied.
