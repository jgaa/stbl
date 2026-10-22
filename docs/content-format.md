# Content Format

This chapter describes how `stbl` parses Markdown content files and metadata headers.

## File Model

Each content file is a Markdown file (`.md`) that may include a metadata header at the top.

`stbl` supports two header styles:

1. Frontmatter-style fenced header (`--- ... ---`)
2. Plain `key: value` lines at the top, ending at first blank line

Both produce the same parsed header data.

## Header Formats

### Frontmatter style

```markdown
---
title: My Article
published: 2026-02-17 10:00
tags: rust, stbl
---

# Body
```

### Plain style

```markdown
title: My Article
published: 2026-02-17 10:00
tags: rust, stbl

# Body
```

## General Parsing Rules

- Keys are case-sensitive.
- Unknown keys are errors by default.
- CLI can downgrade unknown key errors to warnings with `--unknown-header-keys warn`.
- Values are trimmed.
- Empty values are treated as unset for most fields.
- Header lines can include inline comments when `#` is preceded by whitespace.
- Full-line comments starting with `#` are ignored inside headers.
- Header keys may contain ASCII letters/digits plus `-` and `_`.

## Supported Header Fields

### `title`

- Optional string.
- If missing, title is deduced from source path/filename.

### `author`

- Optional comma-separated list.
- Stored as author IDs/names exactly as provided.
- Example: `author: alex, alice`

### `published`

Controls publication status and publish time.

Accepted values:

- Timestamp string
- `no` or `false` (unpublished)
- Empty or missing

Behavior:

- Missing or empty:
  - page is treated as published,
  - publish timestamp is assigned during build,
  - value is eligible for write-back.
- `no`/`false`:
  - page is unpublished,
  - excluded from normal build output,
  - can be included only in preview with `--include-unpublished`.
- Timestamp:
  - page is published at that time.

### `updated`

Optional updated timestamp with disable and fallback behavior.

Accepted values:

- Timestamp string
- `no` or `false` (disable updated time display)
- Missing/empty

Behavior:

- If `updated` is `no|false`: updated display is disabled for that page.
- If `updated` is a timestamp: use that value.
- If `updated` is missing and not disabled: fall back to file modification time (mtime).
- `updated` is not used for article ordering.
- `updated` is not auto-generated or written back to source headers.
- Updated is only shown when it is effectively after published time after timestamp normalization/rounding.

### `tags`

- Optional comma-separated list of strings.
- Empty entries are ignored.
- Example: `tags: rust, static-sites, release-notes`

### `abstract`

- Optional summary text.
- Used by listing templates.
- Also used as the meta description when `description` is unset.

### `description`

- Optional, single-line plain text summary for search engines.
- Rendered as `<meta name="description" content="...">` in the page's `<head>`.
- A non-empty `description` takes precedence over `abstract`; empty values are unset.
- Values are HTML-escaped, without Markdown rendering or automatic truncation.
- Does not change listing excerpts or RSS descriptions.

```text
title: Project guide
description: A practical guide to setting up and maintaining a project.
abstract: Project setup and maintenance advice.
```

Set this on the homepage's content header as well as individual articles and
series indexes. Blog index pages, including paginated pages, use their source
header's description or abstract. This fallback uses the explicit `abstract`
header even when blog excerpts are disabled; automatically derived listing
excerpts are not used. If neither field is set, the meta tag is omitted.
Generated pages without a content header (such as tag listings and redirects)
do not receive a description from this setting.

### `social-image`

- Optional image for Open Graph link previews; overrides the page banner.
- Accepts a filename under `images/`, a path beginning with `images/`, or an
  absolute HTTP(S) image URL. Local paths must stay within `images/`.
- Local files must exist. They are discovered during planning and copied into
  the published site during build, even if they never appear in the page body.
- External image URLs are used directly, without downloading them.

```text
title: Project guide
description: A practical guide to setting up a project.
social-image: images/project-card.jpg
```

The preview image order is `social-image`, the page's banner,
`seo.open_graph.default_image`, then the published frontpage's banner if
`seo.open_graph.use_frontpage_banner` is enabled (default: false). The last two
fallbacks also apply to generated listings such as tag pages. Without an image,
`og:image` is omitted. See [Open Graph settings](templates-and-themes.md#open-graph).

### `template`

- Optional template ID string.
- Must not contain `/`.

Accepted normalized values:

- `landing`
- `blog_index`
- `list-articles` (alias for `blog_index`)
- `page`
- `info`

Accepted legacy aliases:

- `landingpage.html`, `landingpage` -> `landing`
- `frontpage.html`, `frontpage` -> `blog_index`
- `list-articles.html` -> `blog_index`
- `info.html` -> `info`

### `layout`

- Optional page layout selector.
- `layout: landing` enables semantic landing-page containers.
- Any other explicit value, such as `layout: standard`, keeps normal Markdown
  rendering.
- `template: landingpage.html` implies `layout: landing` only when `layout` is
  not explicitly set.

See [Landing Pages](landing-pages.md) for the container syntax.

### `type`

- Optional content type string.
- `info` is treated specially for cover-page behavior.

### `menu`

- Optional string.
- Parsed and preserved for compatibility.

### `icon`

- Optional string.
- Parsed and preserved for compatibility.

### `banner`

- Optional string.
- Parsed as banner identifier/path text in header.
- `verify` expects banner names (not path separators) for local banner resolution checks.

### `banner-credits`

- Optional string for attribution text.

### `comments`

- Optional string selecting page-level comments behavior/provider.

### `part`

- Optional in header, required for fully-specified series part ordering.
- Must be an integer `>= 1` when provided.

Behavior for series parts:

- Missing part numbers are auto-assigned during assemble/build.
- Assigned values may be written back when header exists.
- Duplicate or invalid part values produce errors.

### `uuid`

- Optional UUID.
- Kept for compatibility and tooling.

### `expires`

- Optional timestamp.
- Parsed and preserved for compatibility; the current version does not enforce expiry.

### `sitemap-priority`

- Optional.
- Accepts values in `[0.0, 1.0]` or `-1`.
- `-1` means default/unset.
- Parsed and preserved; the current sitemap renderer does not emit priorities.
- Invalid values produce warnings.

### `sitemap-changefreq`

- Optional.
- Allowed values: `always`, `hourly`, `daily`, `weekly`, `monthly`, `yearly`, `never`.
- Invalid values produce parse errors.
- Parsed and preserved; the current sitemap renderer does not emit change frequencies.

### `exclude_from_blog`

- Optional boolean.
- Default `false`.
- Accepted booleans: `true|false`, `yes|no`, `1|0` (empty behaves as false).
- If true, page is excluded from blog listings/feed.

## Datetime Input Formats

Common accepted formats include:

- `YYYY-MM-DD`
- `YYYY-MM-DD HH:MM`
- `YYYY-MM-DD HH:MM:SS`
- `YYYY-MM-DDTHH:MM`
- `YYYY-MM-DDTHH:MM:SS`
- RFC3339 and RFC2822 timestamps
- Explicit timezone offsets like `+02:00` or `-0500`
- Named timezone suffixes such as `UTC`, `GMT`, `EST`, `EDT`, `CET`, `CEST`

For timestamps without explicit timezone, local timezone is used.

## Write-back Behavior

Fields that may be written back during `build`:

- `published`
- `part`

Rules:

- Write-back is applied during build (non-preview builds).
- Preview builds run write-back in dry-run mode.
- `--no-writeback` disables writes.
- `--commit-writeback` commits applied write-back changes with git.

`updated` is never written back.

## Practical Example

```markdown
---
title: Release 1.2
author: alex
published: 2026-02-17 09:00
updated: 2026-02-17 11:30
tags: release, changelog
template: page
---

## Highlights

- Faster build planning
- Better timestamp handling
```

## Related Chapters

- [Page Types](page-types.md)
- [Timestamps](timestamps.md)
- [Series](series.md)

## Web forms

Use `:::form` YAML containers or `@[form](path="forms/contact.yaml")` for
reusable definitions under `assets/forms/`. Forms support text, email, and
textarea fields; `required` defaults to false. See [Web forms](web-forms.md)
for syntax, examples, and CGI receiver deployment.
