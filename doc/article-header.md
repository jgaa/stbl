## Magic header format

stbl articles begin with an optional **magic header**: a compact key/value block that provides metadata for the article. The header is **the source of truth** for metadata (title, tags, publish date, etc.). stbl may **normalize** the header by filling in missing fields (for example `uuid`, `published`) and writing the updated header back to the file.

### Syntax

* The header consists of **lines** in the form:

  ```
  key: value
  ```

* `key` must match: `^[0-9a-zA-Z-]+$`

* `value` is everything after the first `:` on the line (leading spaces are ignored).

* Empty lines are ignored.

* Unknown keys are ignored (future compatibility).

### Comments with `#`

You can comment out parts of the header using `#`:

* **Full-line comment:** if the first non-whitespace character is `#`, the entire line is ignored.
* **Inline comment:** if `#` is preceded by whitespace, everything from `#` to end-of-line is ignored.
* `#` **inside tokens** (with no preceding whitespace) is preserved (e.g. URL fragments like `https://example.com/page#section`).

Examples:

```text
# title: Disabled title
title: Hello World        # temporary title
banner: https://x/y#v2    # keeps the fragment
```

### Supported fields

| Key                  | Type                     | Meaning                                                                                                 |
| -------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------- |
| `uuid`               | string                   | Unique identifier for the article. If missing/empty, stbl generates one.                                |
| `title`              | string                   | Article title (displayed on pages and feeds).                                                           |
| `abstract`           | string                   | Short summary used on index pages and RSS.                                                              |
| `tags`               | list (comma-separated)   | Tags used for tag pages and filtering. Example: `tags: rust, web, tooling`                              |
| `published`          | datetime or boolean-like | Publish time in `%Y-%m-%d %H:%M` (UTC). Special values `false` or `no` mark the article as unpublished. |
| `updated`            | datetime                 | Last updated time in `%Y-%m-%d %H:%M` (UTC).                                                            |
| `expires`            | datetime                 | Optional expiry time in `%Y-%m-%d %H:%M` (UTC).                                                         |
| `authors`            | list (comma-separated)   | Author IDs/names.                                                                                       |
| `author`             | string                   | Convenience field: if present, it is inserted at the front of `authors`.                                |
| `template`           | string                   | Template name to use for the page (theme-defined).                                                      |
| `type`               | string                   | Content type/category for theme logic.                                                                  |
| `menu`               | string                   | Optional menu label/placement hint (theme-defined).                                                     |
| `banner`             | string                   | Banner image path or URL.                                                                               |
| `banner-credits`     | string                   | Attribution string for the banner.                                                                      |
| `comments`           | string                   | Comment provider/mode (theme-defined).                                                                  |
| `part`               | integer                  | Part number for multi-part articles/series navigation.                                         |

| `sitemap-priority`   | integer                  | Optional sitemap priority (theme-defined semantics).                                                    |
| `sitemap-changefreq` | string                   | Optional sitemap change frequency (e.g. `daily`, `weekly`).                                             |

### Datetime format

All datetimes use the format:

```
YYYY-MM-DD HH:MM
```

Example:

```
published: 2026-01-23 11:30
```

Unless otherwise stated by configuration, timestamps are treated as **UTC**.

### Example header

```text
uuid: 3f97f9c9-32b8-4e58-9d29-4c3b6f6bb2a1
title: Rebuilding stbl in Rust
abstract: Notes from a rewrite focused on maintainability and speed.
tags: rust, static-sites, tooling
published: 2026-01-23 11:30
updated: 2026-01-23 12:05
authors: jgaa
banner: images/rewrite.jpg#v2  # fragment is preserved
banner-credits: Photo by Example Person
comments: off
```
