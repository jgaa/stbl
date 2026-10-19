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
  - `artifacts/robots.txt` overrides the default generated `robots.txt`; it is copied to the output root.
  - Without `site.icon`, regular files matching `artifacts/favicon*` are copied to the output root with their original names. Missing favicon files produce a build warning.
  - `site.icon` retains precedence and its existing SVG-to-PNG conversion.
  - `artifacts/well-known/` is copied recursively to `.well-known/` in the output root, preserving filenames and subdirectories. For example, `artifacts/well-known/security.txt` becomes `.well-known/security.txt`.
  - Files in `artifacts/well-known/` override matching files from `assets/.well-known/`.
  - These root files and `.well-known/` files keep stable names even when asset cache busting is enabled.
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

## Generated crawler files

Builds generate `sitemap.xml` and, unless overridden by `artifacts/robots.txt`,
`robots.txt` in the output root. The default robots file allows crawling and
advertises the absolute sitemap URL derived from `site.base_url`.
For sites hosted under a URL subdirectory, arrange for `robots.txt` to be served
at the domain root so crawlers can discover it.

## Well-known files

Place discovery, verification, or security contact files in `artifacts/well-known/`:

```text
artifacts/well-known/security.txt       → .well-known/security.txt
artifacts/well-known/acme-challenge/id  → .well-known/acme-challenge/id
```

No configuration is required. `scan` and `plan` do not copy files; `build` copies
them as planned assets. Subsequent builds update changed files and remove output
files whose sources were deleted. Configure hosting to serve `.well-known/`
at the domain root, including when the site is hosted under a URL subdirectory.
