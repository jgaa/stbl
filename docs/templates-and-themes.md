# Templates and Themes

This chapter explains how `stbl` resolves templates, CSS, and theme settings.

## Theme Variant

Theme selection is controlled by:

```yaml
theme:
  variant: stbl
```

Notes:

- `default` is treated as alias for `stbl`.
- Empty variant also resolves to `stbl`.

## Product landing theme

`theme.variant: product` selects a product landing theme with bold headings,
an inline logo and menu, quiet navigation, responsive hero images, and primary
and secondary link buttons. Its blue default palette is also available as the
`product` color preset. Existing color presets and custom overrides work with
this theme; button text is chosen for contrast against the resolved accent.

An optional action sits beside the logo and menu:

```yaml
theme:
  variant: product
  header:
    action:
      title: Get Started
      href: download
```

The action is a link. Relative page destinations use `UrlMapper` and the site
base path, so `download` follows the configured URL style. External URLs,
root paths and fragments retain their meaning. The product header keeps the
logo, navigation and action inline on desktop; mobile uses a native `details`
menu. Buttons and navigation require no JavaScript. Responsive custom video
posters use a small shared script, with a generated poster when scripts are
disabled.

Use `@[button]` in hero and callout content; see [Landing Pages](landing-pages.md).

## Theme Config Surface

Main theme fields in `stbl.yaml`:

- `theme.variant`
- `theme.max_body_width`
- `theme.breakpoints.desktop_min`
- `theme.breakpoints.wide_min`
- `theme.colors.*`
- `theme.nav.*`
- `theme.header.layout`
- `theme.header.menu_align`
- `theme.header.title_size`
- `theme.header.tagline_size`
- `theme.header.action.title`
- `theme.header.action.href`
- `theme.wide_background.*`
- `theme.color_scheme.*`

These values feed generated CSS variables and template rendering context.

## Template Set

Core templates expected by the renderer:

- `templates/base.html`
- `templates/page.html`
- `templates/partials/blog_index.html`
- `templates/tag_index.html`
- `templates/series_index.html`
- `templates/partials/list_item.html`
- `templates/partials/header.html`
- `templates/partials/footer.html`

For comment providers, templates such as `templates/disqus.html` and partial variants may also be used.

## Asset and Template Override Order

Assets are merged with later sources overriding earlier ones:

1. Embedded `stbl` theme assets
2. Embedded selected variant assets (if different)
3. `<site>/stbl/templates/<variant>/` mapped under `templates/`
4. `<site>/stbl/css/<variant>/` mapped under `css/`
5. `<site>/stbl/assets/<variant>/` mapped at asset root
6. `<site>/assets/` mapped at asset root

Practical guidance:

- Use `assets/` for most site-level overrides.
- Use `stbl/.../<variant>/` when you need variant-scoped overrides.

## CSS Variables (`artifacts/css/vars.css`)

During build, `stbl` generates `artifacts/css/vars.css`.

Generation flow:

- Load theme defaults (`stbl/colors` YAML for the active variant, fallback to `stbl`).
- Merge with `stbl.yaml` theme overrides.
- Emit `:root` CSS variables (layout, colors, nav colors, wide background settings).

Important:

- `css/vars.css` from embedded assets is intentionally not copied as a static asset.
- Generated `artifacts/css/vars.css` is the authoritative runtime vars file.

## Color Presets Workflow

CLI support:

- `stbl_cli apply-colors --list-presets`
- `stbl_cli apply-colors <name>`
- `stbl_cli apply-colors --from-base ...`
- `stbl_cli show-color-themes --open`

This updates `theme.colors`, `theme.nav`, `theme.wide_background`, and `theme.color_scheme` in `stbl.yaml`.

## Comment Template Resolution

When a comment template is requested, lookup is roughly:

- theme-specific site overrides under `stbl/templates/<variant>/` (with `stbl` fallback),
- direct site paths (relative to project root),
- site `templates/` shortcuts,
- embedded template candidates.

Paths must stay within site root (path escape is rejected).

## Safe Customization Strategy

Recommended order:

1. Start with config-only tuning (`theme.*` in `stbl.yaml`).
2. Use `apply-colors` for palette changes.
3. Override CSS in `assets/css/...` when needed.
4. Override template files only when markup changes are required.

## Verification and Debugging

Useful commands:

```sh
stbl_cli verify
stbl_cli build --out ./out
```

Then inspect:

- `out/artifacts/css/vars.css`
- `out/artifacts/css/*.css`
- `out/*.html` and listing pages

If an override is not applied, check:

- path location (`assets/` vs `stbl/.../<variant>/`),
- selected `theme.variant`,
- filename/path match to expected template asset names.

## Related Chapters

- `docs/project-structure.md`
- `docs/cli.md`
- `docs/content-format.md`
