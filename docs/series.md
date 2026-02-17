# Series

A series is an ordered set of related articles under one directory.

## Directory Shape

A directory is treated as a series when it contains `index.md` below `articles/`.

Example:

```text
articles/my-series/
├── index.md
├── part-1.md
└── part-2.md
```

Rules:

- `index.md` is the series cover/index page.
- Other `.md` files in that same directory are series parts.
- A series without an index page is not assembled as a series.

## Discovery and Classification

During scan/walk:

- top-level pages are `DocKind::Page`,
- `.../index.md` inside a detected series directory is `DocKind::SeriesIndex`,
- other files in that directory are `DocKind::SeriesPart`.

## Part Numbers (`part`)

Part numbers control ordering.

Accepted explicit values:

- integer `>= 1`

Validation:

- invalid/non-integer/`< 1` -> error,
- duplicate number in same series -> error.

Auto-assignment when missing:

- missing/empty part numbers are assigned during assemble,
- existing valid part numbers are preserved,
- missing ones are sorted by file `mtime`,
- each gets the lowest free positive number.

After assignment:

- parts are sorted by `part_no`.
- if sequence has holes (for example `1,2,4`), a warning is emitted.

## Write-back Behavior

Assigned part numbers are eligible for header write-back when:

- the source file had a parseable header section.

Write-back occurs in build mode (not preview), subject to global write-back flags.

## Publishing Behavior

Series part visibility follows normal published rules:

- unpublished parts are excluded from public listings and series page part lists.
- preview with unpublished inclusion follows global preview rules.

Series index itself must also pass normal listing visibility checks to appear as a rollup item.

## URLs and Output Paths

Series index pages always use directory-style output mapping:

- href: `series-slug/`
- output: `series-slug/index.html`

This is true even when global `site.url_style` is `html`.

Series parts use normal page URL mapping based on global URL style.

## Rendering Behavior

Series index page rendering:

- uses dedicated series index template,
- lists published parts with title + published date.

Series part rendering:

- rendered as normal pages,
- includes series navigation block with:
  - link to series index,
  - list of published parts,
  - current part highlighted.

## Blog/Tag Listing Behavior

Series can appear in article listings as aggregate items:

- listing item sort date is based on latest published part,
- tags are merged from series index and published parts,
- “latest parts” preview is limited by `blog.series.latest_parts` (default 3).

Series parts are still regular article pages and can be linked directly.

## Header Constraints and Warnings

`verify` warns on common series mistakes:

- `part` set on a non-series page,
- series pages marked as `template: info` or `type: info`.

## Example

```markdown
# articles/my-series/index.md
title: My Series
abstract: Multi-part deep dive

Intro text...
```

```markdown
# articles/my-series/part-a.md
title: Part A
part: 1
published: 2026-02-17 09:00

Body...
```

```markdown
# articles/my-series/part-b.md
title: Part B
# part omitted: may be assigned during build
published: 2026-02-18 09:00

Body...
```

## Related Chapters

- `docs/project-structure.md`
- `docs/content-format.md`
- `docs/page-types.md`
