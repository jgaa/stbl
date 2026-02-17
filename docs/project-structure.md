# Project Structure

This chapter describes the site directory layout expected by `stbl_cli`.

## Minimal Site Layout

After `stbl_cli init`, a typical project looks like:

```text
my-site/
├── stbl.yaml
├── articles/
│   ├── index.md
│   ├── about.md
│   └── contact.md
├── artifacts/
├── assets/
│   └── README.md
├── images/
└── video/
```

## Required Paths

- `stbl.yaml`
  - Main site configuration.
- `articles/`
  - Markdown content root.
  - Must contain `index.md` for the front page.

## Optional Paths

- `assets/`
  - Site-specific asset overrides (CSS/templates/images/icons/etc).
  - Overlays embedded theme assets.
- `images/`
  - Source images referenced by content/config.
- `video/`
  - Source video media.
- `files/`
  - Static passthrough files copied as-is to output under `files/`.
- `artifacts/`
  - Common place for generated/static runtime assets referenced by config/publish workflows.
  - Usually produced in output; a project-level `artifacts/` directory may also be used in workflows.
- `stbl/`
  - Optional advanced override tree for theme-scoped assets.

## Content Layout (`articles/`)

### Standalone pages

Simple pages live directly under `articles/`:

```text
articles/
├── index.md
├── about.md
└── contact.md
```

### Blog/article grouping

You can group article files in subdirectories:

```text
articles/
├── _blog/
│   ├── post-one.md
│   └── post-two.md
└── index.md
```

### Series

A series is a directory containing its own `index.md` plus part files:

```text
articles/
└── my-series/
    ├── index.md
    ├── part-1.md
    └── part-2.md
```

## Asset Override Resolution

At build time, assets are overlaid in precedence order (later sources win):

1. Embedded default theme assets
2. Embedded selected theme variant assets (if different from default)
3. `<site>/stbl/templates/<theme-variant>/` mapped to template paths
4. `<site>/stbl/css/<theme-variant>/` mapped to css paths
5. `<site>/stbl/assets/<theme-variant>/` mapped to asset root
6. `<site>/assets/` mapped to asset root

Practical meaning:

- Put normal overrides in `assets/`.
- Use `stbl/.../<theme-variant>/` only when you need variant-specific overrides.

## Output vs Source Paths

### Source tree

- You edit in the project root (`articles/`, `assets/`, `stbl.yaml`, etc).

### Build output tree

- Generated HTML/assets are written to build output directory (default cache location unless `--out` is used).
- `files/` from source is copied recursively into output `files/`.
- Generated web assets are typically under output `artifacts/` (for example CSS files).

## Path Conventions

- Use relative paths in config/content (for example `images/logo.svg`).
- Avoid absolute filesystem paths in `stbl.yaml`.
- Keep filenames URL-friendly (`lowercase-with-dashes.md`) to reduce surprising URLs.

## Common Mistakes

Missing front page:
- `articles/index.md` is required for expected root page behavior.

Assets not taking effect:
- Confirm file is under `assets/` or correct `stbl/.../<theme-variant>/` path.

Unexpected stale files in output:
- Use `stbl_cli clean` and rebuild.

## Related Chapters

- `docs/content-format.md`
- `docs/page-types.md`
- `docs/templates-and-themes.md`
