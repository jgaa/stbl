# Configuration Reference

The site configuration file is **`stbl.yaml`**, in the site root beside
`articles/`. The CLI does not load a file named `config.yaml`. Use
`stbl_cli --source-dir /path/to/site …` to select another site root.
`stbl_cli init` creates a commented configuration you can customize.

Settings below describe the current implementation. YAML indentation matters;
quote CSS colors (otherwise `#` begins a comment). Omit optional sections you do
not need. Run `stbl_cli verify --strict` after editing: unknown keys produce
warnings, which strict verification treats as failures. Build validation also
checks content and referenced assets.

## Minimal configuration

```yaml
site:
  id: example-site
  title: Example Site
  base_url: https://example.com/
  language: en
```

All four fields are required, non-empty strings. Keep `site.id` stable and unique
among your sites: it selects the default cache/output directory
`~/.cache/stbl/<site.id>/`. Set `base_url` to the public production URL, including
any mount path, for example `https://example.com/blog/`. Canonical links, Open
Graph URLs, RSS links, and the sitemap use it even during local preview.
`build --base-url URL` overrides it for a single build without editing YAML.

## Site metadata and URLs

| Key | Default | Behavior |
| --- | --- | --- |
| `site.id` | Required | Site identity used for cache/output selection. |
| `site.title` | Required | Site title in templates and feeds. |
| `site.base_url` | Required | Public absolute site URL, including its mount path. |
| `site.language` | Required | HTML language and RSS language, for example `en`. |
| `site.tagline` | Unset | Short text under the title; also RSS channel description. |
| `site.logo` | Unset | Local logo asset, for example `images/logo.svg`. |
| `site.icon` | Unset | Local SVG converted to a 32×32 PNG site icon. Takes precedence over fallback favicon files in `artifacts/`. |
| `site.copyright` | `Copyright {YEAR} by {site.title}` | Literal footer text; custom text is not interpolated. |
| `site.timezone` | UTC for display | Named display timezone, for example `Europe/Paris`; invalid names fall back to UTC. Timestamp input without a timezone uses the local timezone. |
| `site.url_style` | `html` | One of the URL styles below. |
| `site.macros.enabled` | `true` | Expand content macros such as includes, TOC, buttons, and listings. |

`site.abstract` is a compatibility fallback for `site.tagline`. It does not
provide a default page meta description. Set `description` or `abstract` in each
content header; see [Content Format](content-format.md#description).

| URL style | Example link | Output |
| --- | --- | --- |
| `html` | `guide.html` | `guide.html` |
| `pretty` | `guide/` | `guide/index.html` |
| `pretty_with_fallback` | `guide/` | `guide/index.html`, plus `guide.html` redirect |

The root page uses the site root. Series indexes always use directory URLs.
Canonicals follow the mapped primary URL; redirects canonicalize to their target.

## Navigation

```yaml
menu:
  - title: Home
    href: ./
  - title: About
    href: about
  - title: External resource
    href: https://example.org/
```

`menu` is an ordered list of entries with a required `title` and optional `href`.
An omitted `href` defaults to the lowercased title plus `.html`; explicit logical
page destinations such as `about` are preferable. Internal page links follow the
URL mapper. External URLs and fragments retain their meaning.

For compatibility, `site.nav` accepts entries with `label` and optional `href`.
When present, it takes precedence over `menu` for rendered navigation. Without
it, navigation is derived from `menu`.

## Themes, layout, and colors

```yaml
theme:
  variant: stbl
  max_body_width: 72rem
  breakpoints:
    desktop_min: 768px
    wide_min: 1400px
  header:
    layout: stacked
    menu_align: left
    title_size: 1.3rem
    tagline_size: 1rem
  colors:
    accent: "#2456a6"
    link: "#2456a6"
```

`variant` defaults to `stbl`; `default` is an alias. Embedded variants are
`stbl`, `minimal`, `liberty`, `paper`, `mono`, and `product`. The layout values
above are the configuration defaults; theme templates determine how they are
presented. Header `layout` accepts `stacked` or `inline`, and `menu_align`
accepts `left`, `center`, or `right`.

Optional `theme.header.action` requires both `title` and `href` and adds a link
button in the product header. Other embedded themes do not render this action.

`theme.colors` accepts these optional CSS color overrides:
`bg`, `fg`, `heading`, `title_fg`, `accent`, `link`, `muted`, `surface`, `border`,
`link_hover`, `code_bg`, `code_fg`, `quote_bg`, `quote_border`, and `wide_bg`.
Unspecified colors are resolved from the selected theme and color scheme;
`title_fg` falls back to the heading color. `theme.nav` accepts `bg`, `fg`, and
`border` overrides.

`theme.wide_background` accepts optional `color`, `image`, `style` (`cover` or
`tile`), `position` (CSS background position), and `opacity` (0.0–1.0). Defaults
come from the selected theme. Local background images must exist and are
included in image discovery.

Color scheme metadata lives under `theme.color_scheme`:

| Key | Values |
| --- | --- |
| `name` | Optional preset/derived scheme name. |
| `mode` | `auto`, `light`, or `dark`. |
| `source` | `preset` or `derived`. |
| `base` | Optional `bg`, `fg`, `accent`, `link`, and `heading` colors. |

Use `stbl_cli apply-colors --list-presets` to list available presets and
`stbl_cli apply-colors NAME` to write a resolved palette. To derive a palette,
use `stbl_cli apply-colors --from-base --bg '#ffffff' --accent '#2456a6'`.
`build --theme NAME` and `build --color-theme NAME` apply temporary overrides.
See [Templates and Themes](templates-and-themes.md) for overrides and generated CSS.

## Syntax highlighting

| Key | Default | Behavior |
| --- | --- | --- |
| `syntax.highlight` | `true` | Highlight fenced code blocks; false emits escaped plain code. |
| `syntax.theme` | `GitHub` | Highlighting theme; empty uses `GitHub`, unknown themes fall back to plain code. |
| `syntax.line_numbers` | `true` | Include line-number markup. |

## Media

```yaml
media:
  images:
    widths: [94, 128, 248, 360, 480, 640, 720, 950, 1280, 1440, 1680, 1920, 2560]
    quality: 90
  video:
    heights: [360, 480, 720, 1080]
    poster_time: 1
```

These are the defaults. Widths/heights are pixel targets; variants larger than
the source are skipped. Image quality is 1–100 for JPEG/WebP encoding, and does
not affect PNG. `poster_time` is a non-negative integer in seconds, also accepted
as a string such as `"2s"`. Video processing requires `ffmpeg` on `PATH`.

Banners use the shared image pipeline and `media.images` settings. The legacy
`banner.widths`, `banner.quality`, and `banner.align` fields are accepted and
stored, but do not currently change processing or alignment. Use the content
`banner` header to select a banner. See [Media](media.md) for syntax, custom video
posters, output formats, and compile-time AVIF support.

## Assets, SVG safety, and footer

| Key | Default | Behavior |
| --- | --- | --- |
| `assets.cache_busting` | `false` | Add content hashes to generated asset filenames and rewrite references. Root discovery files retain stable names. |
| `security.svg.mode` | `warn` | SVG checks during asset copying; modes below. |
| `footer.show_stbl` | `true` | Show the generated-by/date footer text. |

SVG modes are `off` (copy unchanged without scanning), `warn` (report and copy),
`fail` (reject detected unsafe content), and `sanitize` (rewrite to a safer
subset). These checks do not constitute general sanitization of arbitrary page
HTML. See [Project Structure](project-structure.md) for asset roots, favicons,
robots overrides, and `.well-known/` files.

Compression is selected with build flags (`--precompress`, `--fast-compress`),
not YAML. Cache paths, worker counts, output paths, and preview settings are also
CLI options; see [CLI Reference](cli.md).

## Authors

```yaml
people:
  default: alex
  entries:
    alex:
      name: Alex Example
      email: alex@example.com
      links:
        - id: website
          name: Personal website
          url: https://example.org/
          # icon: icons/website.svg
```

If `people` is present, `entries` must be a non-empty mapping of author IDs to
entries. Each entry requires `name`; `email` and `links` are optional. Each link requires
`id`, `name`, and `url`, with an optional icon override. `default` must name an
existing entry; if omitted, the first ID in sorted order is selected. Content
headers select authors using `author: alex` or a comma-separated list.

## Dates

```yaml
system:
  date:
    format: "%B %-d, %Y at %H:%M %Z"
    roundup_seconds: 0
```

When `system.date` is provided, both `format` and `roundup_seconds` are required.
If the section is omitted, the format above and zero rounding are used
(strftime format syntax). Despite the
name, a positive `roundup_seconds` rounds timestamps **down** to a multiple of
that many seconds before display/comparison. Zero disables rounding. See
[Timestamps](timestamps.md) for input timezone handling and updated-time rules.

## Blog listings and series

```yaml
blog:
  abstract:
    enabled: true
    max_chars: 200
  pagination:
    enabled: false
    page_size: 10
  series:
    latest_parts: 3
```

These are the defaults. `abstract.enabled` controls listing summaries, with
`max_chars` greater than zero when enabled. It does not disable metadata's
explicit abstract fallback. Pagination requires a positive `page_size` when
enabled. `latest_parts` limits previews in series rollup entries.

Legacy `blog.page_size` is accepted: when `blog.pagination` is absent, providing
it enables pagination. Prefer the explicit `pagination` section for new sites.
See [Page Types](page-types.md) and [Series](series.md).

## RSS

```yaml
rss:
  enabled: true
  max_items: 20
  ttl_channel: 60
  # ttl_days: 30
```

RSS is disabled by default. Enabling it requires an explicit `max_items`; the
example chooses 20. `ttl_channel` is an optional refresh hint in minutes;
omitting it or setting zero omits the RSS `<ttl>`. `ttl_days`, if set, must be
positive and filters out older items before `max_items` is applied. Items are
ordered by publication time (using updated time only as a fallback when no
publication time is available). The output is `rss.xml`.

## Search metadata and Open Graph

```yaml
seo:
  open_graph:
    # default_image: images/social-card.jpg
    use_frontpage_banner: false
```

Embedded themes generate canonical links and Open Graph tags without enabling
an option. `use_frontpage_banner` defaults to false. `default_image` is optional;
it accepts a filename under `images/`, a path beginning `images/`, or an absolute
HTTP(S) URL. Local image references must exist and stay inside `images/`.
External images are linked without downloading them.

Image selection is: page `social-image` header → page banner → configured
`default_image` → published root frontpage banner when enabled. The last fallback
uses `articles/index.md`, including a homepage rendered as a blog index. Local
images are published through planned image tasks; metadata links to the original
image using an absolute URL. Without an image, `og:image` is omitted.

Descriptions come from the page's `description` header, then its explicit
`abstract`. No description is generated from body excerpts, and there is no
site-wide description setting. Generated tag listings have no source header
and therefore no description. See [Content Format](content-format.md) and
[Templates and Themes](templates-and-themes.md#open-graph).

Builds generate `sitemap.xml` and a default `robots.txt` advertising it; a source
`artifacts/robots.txt` overrides the latter. The current sitemap emits locations
and available modification times. The accepted `seo.sitemap.priority` fields
(`frontpage`, `article`, `series`, `tag`, `tags`) are stored but not emitted.
The content `sitemap-priority` and `sitemap-changefreq` headers are likewise not
emitted. There is no built-in JSON-LD, Twitter Card, or `llms.txt` generator.

## Comments

```yaml
comments:
  default: discussion
  discussion:
    template: templates/discussion.html
    src: https://comments.example.org/embed.js
```

A provider requires `template`: inline HTML or a template asset path. Other
provider keys are scalar variables for that snippet, referenced with the
provider prefix, for example `{{discussion-src}}`. The content `comments` header
can choose a provider or disable it with `no`. This is a template integration;
stbl does not provide a comment storage/moderation service. Third-party snippets
may require JavaScript. See [Templates and Themes](templates-and-themes.md).

## Publishing

```yaml
publish:
  command: "rsync -a {{local-site}}/ {{destination}}/"
```

Publishing is optional and runs only when building with `--publish-to DEST`.
`{{local-site}}` is replaced by the resolved output directory and
`{{destination}}` by the CLI destination. The configured command is executed
locally; choose it for your hosting environment. See [CLI Reference](cli.md#build).

## Forms and compatibility settings

Forms are defined in content containers or reusable YAML under `assets/forms/`;
they do not have a top-level site configuration section. The optional CGI
receiver has a **separate private configuration**, outside the published output.
See [Web Forms](web-forms.md#private-receiver-configuration) for its full example,
limits, and deployment instructions.

Legacy top-level `chroma` and `plyr` sections are accepted for compatibility but
have no rendering effect. Use `syntax` and the native video pipeline instead.
The content `expires` field is parsed but expiry is not currently enforced.
To migrate `stbl.conf`, run `stbl_cli upgrade --source-dir /path/to/site`;
the result is a `stbl.yaml` file, not `config.yaml`.
