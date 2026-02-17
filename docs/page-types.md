# Page Types

`stbl` distinguishes between **cover pages** and **articles** for rendering and listing behavior.

## Definitions

## Cover pages

A page is a cover page if any of the following is true:

- It is the root front page (`articles/index.md`).
- Header `template` resolves to `info`.
- Header `type` is `info`.
- It is a generated tag page (`tags/...` listing pages).

## Articles

Articles are pages that appear in article/blog listings, including:

- standalone published content pages,
- published series parts,
- published series entries shown as series items in listings.

Practical rule: if a page is included by blog/tag listing logic, it is treated as article content.

## Header Metadata Display

Cover pages do **not** display source-header article metadata in the page header:

- no published date,
- no updated date,
- no author block,
- no tag list.

Articles may display those fields according to normal timestamp/author/tag rules.

## Listing Inclusion Rules

Pages are excluded from blog/article listings when any of these apply:

- unpublished (`published: no|false`),
- `exclude_from_blog: true`,
- classified as cover page,
- uses template `blog_index`, `landing`, or `info`,
- is the listing source page itself (for self-exclusion in some contexts).

Series behavior:

- Series index pages act as collection containers.
- Published series parts are considered article content.
- Series can appear in listings as aggregated items when they have published parts.

## Examples

Cover page examples:

```text
articles/index.md
articles/about.md        # when template: info
articles/contact.md      # when type: info
generated tags/rust/...  # tag listing pages
```

Article examples:

```text
articles/post-1.md
articles/news/release.md
articles/my-series/part-1.md
```

## Recommendations

- Use `template: info` (or `type: info`) for static informational pages that should not be treated as articles.
- Use normal article templates for content you want in blog/tag listings.
- Use `exclude_from_blog: true` for edge cases where a page should remain published but hidden from listings.

## Related Chapters

- `docs/content-format.md`
- `docs/timestamps.md`
- `docs/series.md`
