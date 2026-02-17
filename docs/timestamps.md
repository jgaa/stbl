# Timestamps

This chapter defines how `stbl` handles `published` and `updated` timestamps.

## Overview

`stbl` stores timestamps as Unix seconds internally and applies normalization/display rules at render time.

Key behaviors:

- `published` controls publication status.
- `updated` is optional and can be explicitly disabled.
- If `updated` is not disabled and not set, it falls back to file modification time (`mtime`).
- Updated is shown only when it is effectively after published time.

## `published` Semantics

Accepted header values:

- Timestamp string
- `no` / `false`
- Empty/missing

Behavior:

- Timestamp -> page is published at that time.
- `no|false` -> page is unpublished.
- Empty/missing -> page is treated as published, timestamp is assigned during build, and may be written back.

## `updated` Semantics

Accepted header values:

- Timestamp string
- `no` / `false`
- Empty/missing

Behavior:

- Timestamp -> explicit updated time.
- `no|false` -> updated display disabled for that page.
- Empty/missing (and not disabled) -> fallback to source file `mtime`.

Notes:

- `updated` is never auto-generated into headers.
- `updated` is never written back.
- `updated` is not used for article sorting.

## Datetime Input Formats

Supported inputs include:

- `YYYY-MM-DD`
- `YYYY-MM-DD HH:MM`
- `YYYY-MM-DD HH:MM:SS`
- `YYYY-MM-DDTHH:MM`
- `YYYY-MM-DDTHH:MM:SS`
- RFC3339 and RFC2822
- Explicit numeric timezone offsets (`+02:00`, `-0500`)
- Timezone suffixes (`UTC`, `GMT`, `EST`, `EDT`, `CET`, `CEST`, etc.)

When no timezone is present, local timezone is used while parsing.

## Normalization and Rounding

Before display/compare, timestamps are normalized using:

- `system.date.roundup_seconds` from `stbl.yaml`

Rule:

- `0` (default): no rounding.
- `>0`: timestamp is rounded down to the nearest multiple of `roundup_seconds`.

Both `published` and `updated` use the same normalization.

## Updated Display Condition

After normalization:

- if `updated <= published`, updated is hidden
- if `updated > published`, updated is shown
- if published is missing and updated exists, updated may still be shown

This ensures “Updated” appears only when meaningfully later than publish time under current rounding settings.

## Timezone and Display Format

Display uses:

- `site.timezone` (if configured and valid), otherwise UTC
- `system.date.format` (if configured), otherwise default format

Default display format:

```text
%B %-d, %Y at %H:%M %Z
```

Machine-readable timestamp attributes use RFC3339.

## mtime Fallback Details

When `updated` is missing and not disabled:

- source file `mtime` is converted to Unix seconds,
- non-positive values are ignored,
- resulting value is treated as candidate updated timestamp.

This fallback is applied during assemble and then flows through normal rounding/display gating.

## Write-back Rules

Write-back can modify:

- `published`
- `part`

Write-back does not modify:

- `updated`

In preview mode, write-back is dry-run only.

## Examples

### Example 1: Explicit updated shown

```yaml
published: 2026-02-17 10:00
updated: 2026-02-17 12:00
```

Result: updated shown (assuming no rounding collapse).

### Example 2: Updated disabled

```yaml
published: 2026-02-17 10:00
updated: false
```

Result: updated hidden, no mtime fallback.

### Example 3: mtime fallback

```yaml
published: 2026-02-17 10:00
# updated omitted
```

Result: updated uses file `mtime` unless disabled.

### Example 4: Rounded equality hides updated

If `roundup_seconds: 3600` and:

- published = `10:05`
- updated = `10:55`

Both normalize to `10:00`; updated is hidden.

## Where Timestamps Are Shown

- Page header metadata (non-cover pages).
- Blog/tag/series listing metadata (subject to template decisions).
- Feeds/sitemap fields (using feed/sitemap-specific mapping rules).

Cover pages suppress page-header timestamps.

## Related Chapters

- `docs/content-format.md`
- `docs/page-types.md`
- `docs/series.md`
