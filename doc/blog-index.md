# blog_index semantics

## Overview

Pages using template `blog_index` render a blog listing with optional pagination.
The listing is built from a deterministic feed generated in `stbl_core`.

## Exclusion rules

An entry is excluded from the blog feed if any of the following hold:

- `exclude_from_blog: true`
- `template: info`
- `type: info`
- `template: blog_index`
- the entry is the current page being rendered
- `published: false`
- it is the frontage (root index.html)

## Series roll-up

Series content is represented as a single roll-up entry:

- One entry per series.
- `sort_date` is the most recent published timestamp among parts.
- The entry abstract is derived from the series index content
  (MVP: first non-empty paragraph).
- Latest parts list includes the newest parts only.

## Configuration

`blog.series.latest_parts` controls how many parts are listed per series.
Default: 3.

`blog.pagination.enabled` toggles pagination.
Default: false.

`blog.pagination.page_size` controls pagination size.
Default: 10.

## Abstracts

Blog index entries can display a short abstract.

- If `header.abstract` is present and non-empty, it is used verbatim.
- Otherwise, an abstract is derived from the first non-empty paragraph of the body.
- Markdown is rendered to HTML, tags are stripped to plain text, and whitespace is collapsed.
- The text is truncated to `blog.abstract.max_chars` with an ellipsis if needed.

Configuration:

- `blog.abstract.enabled` (default true)
- `blog.abstract.max_chars` (default 200)

## Sorting

The feed is sorted deterministically:

1. `sort_date` descending
2. Tie-breaker: logical key (stable) ascending
