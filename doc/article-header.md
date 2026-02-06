## Article Header Specification

Each content document may begin with a **header block** containing metadata.
The header is the **source of truth** for content metadata.

### Header block format

The header is a sequence of `key: value` lines at the top of the file, terminated by the first empty line.

Example:

```
title: My Article
author: jgaa, contributor2
published: 2024-10-01 12:00
tags: rust, stbl
part: 2

# Article body starts here
```

---

## General parsing rules

* Header keys are **case-sensitive**
* **Unknown keys are errors by default**

  * CLI option may downgrade unknown-key errors to warnings
* Values are trimmed of surrounding whitespace
* Empty values are treated as “unset”

---

## Supported fields

### `title`

* Optional
* Free text
* If omitted, page title falls back to the filename in default templates

---

### `author`

* Optional
* Comma-separated list of **author IDs**
* IDs must exist in the `people` directory in `stbl.yaml`

Example:

```
author: jgaa, alice
```

---

### `published`

Controls whether and when a document is published.

Accepted values:

* `no` / `false`

  * Document is **unpublished**
  * Excluded from site generation by default
* Date/time string
* Empty or missing

Behavior:

* If `published` is **missing or empty**:

  * A publish timestamp is generated during build
  * The header is **written back** to the document
* If `published` is `no|false`:

  * Document is excluded unless:

    * `--preview` **and**
    * `--include-unpublished` are both set
  * Publish/deploy must refuse when unpublished content is included

Datetime rules:

* **Flexible input**, strict output
* Generated timestamps are written in **ANSI format**
* Timezone:

  * Use `site.timezone` from config if present
  * Otherwise use **local timezone**

---

### `updated`

* Optional
* Date/time string
* Represents the **last update time**
* Overrides filesystem modification time

Rules:

* If present: use this value
* If missing: use file modification time
* **Never auto-generated**
* **Never written back**

---

### `tags`

* Optional
* Comma-separated list of tag strings

---

### `abstract`

* Optional
* Short summary text used for blog listings
* If missing and blog abstracts are enabled, an abstract is derived from the body

---

### `template`

* Optional
* String
* Must be a filename without `/`
* Unknown values produce a warning or error depending on template policy

Valid values (normalized):
* `landing`
* `blog_index`
* `list-articles` (alias for `blog_index`)
* `page`
* `info`

Legacy mappings (accepted inputs → normalized value):
* `landingpage.html` → `landing`
* `frontpage.html` → `blog_index`
* `list-articles.html` → `blog_index`
* `info.html` → `info`

---

### `type`

* Optional
* String
* Stored as content type, available to templates and tooling
* Not used by default templates

Special types:
  - info

---

### `menu`

* Optional
* String
* Legacy field, parsed and stored for compatibility
* Not used by default templates

---

### `icon`

* Optional
* String
* Legacy field, parsed and stored for compatibility
* Not used by default templates

---

### `banner`

* Optional
* String
* Banner image name or `images/...` path
* Used to render a banner image on supported templates

---

### `banner-credits`

* Optional
* String
* Banner attribution text
* Parsed and stored for template use

---

### `comments`

* Optional
* String
* Legacy field, parsed and stored for compatibility
* Not used by default templates

---

### `part`

Used for ordered series content.

* Integer **≥ 1**
* Required for series parts

Behavior:

* If missing or empty:

  * Assigned during generation
  * Assignment rules:

    * Existing part numbers are **never changed**
    * Missing parts are sorted by file modification time
    * Numbers are assigned starting at the lowest free index
  * Assigned values are **written back**
* Diagnostics:

  * Error if value `< 1` or not an integer
  * Warning if sequence contains holes (e.g. `1,2,3,5`)

---

### `uuid`

* Optional
* UUID string
* Accepted for backward compatibility
* Not required
* Not generated
* Not used by default templates

---

### `expires`

* Optional
* Date/time string
* Parsed and stored for template or tooling use
* Not used by default templates

---

### Sitemap fields

#### `sitemap-priority`

* Optional
* Must be a valid sitemap priority value or -1 (default)
* `-1` is treated as if the header was not set

#### `sitemap-changefreq`

* Optional
* Must be a valid sitemap change frequency value

Invalid sitemap values are **errors**.


### `exclude_from_blog`

* Optional
* Defaults to false
* Boolean values: `true|false`, `yes|no`, `1|0`

If true, the page must not appear in the blog feed regardless of template or location.

Example

```
---
title: About
template: info
exclude_from_blog: true
---
```

---

## Write-back behavior

The following fields may be written back during build:

* `published`
* `part`

Rules:

* Write-back happens during **build**
* Default: files are modified but **not committed**
* An INFO message is printed **at the end of output**
* CLI options:

  * `--no-writeback`: allowed **only in preview mode**
  * `--commit-writeback`: automatically commits changes

---

## Diagnostics severity

| Condition                       | Severity |
| ------------------------------- | -------- |
| Unknown header key (default)    | Error    |
| Unknown header key (downgraded) | Warning  |
| Invalid `part` value            | Error    |
| Missing `part` (series)         | Info     |
| Holes in part sequence          | Warning  |
| Invalid sitemap fields          | Error    |
