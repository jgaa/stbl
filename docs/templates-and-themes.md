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
