# Macros

Macros let you insert dynamic or structured content directly into Markdown pages.
They are written using the syntax:

```
@[name](optional arguments)
```

Some macros also support a **block form** with a body:

```
@[name](optional arguments)
Markdown content…
@[/name]
```

Macros are expanded at build time and work in posts, pages, and included content.

---

## `@[blogitems]` — List latest blog posts

Inserts a list of the most recent blog posts.

### Syntax

```
@[blogitems]
@[blogitems](items=5)
```

### Arguments

| Name  | Type | Default | Description             |
| ----- | ---- | ------- | ----------------------- |
| items | int  | 3       | Number of posts to show |

### Behavior

* Uses the same visibility rules as the blog index (drafts, future posts, excluded posts are ignored)
* Sorted by publication date (newest first)
* Shows title and abstract/excerpt

### Example

```
## Latest posts
@[blogitems](items=5)
```

---

## `@[toc]` — Table of contents

Generates a table of contents based on headings in the current page.

### Syntax

```
@[toc]
@[toc](min=2,max=3)
@[toc](title="On this page")
```

### Arguments

| Name  | Type   | Default | Description                             |
| ----- | ------ | ------- | --------------------------------------- |
| min   | int    | 2       | Minimum heading level (e.g. `2` = `##`) |
| max   | int    | 3       | Maximum heading level                   |
| title | string | none    | Optional title above the TOC            |

### Notes

* Headings must exist **below** the macro position
* Links match the generated heading IDs exactly

---

## Callout macros — `@[note]`, `@[tip]`, `@[info]`, `@[warning]`, `@[danger]`

Render styled callout boxes for highlighting important content.

### Syntax (block macro)

```
@[note](title="Heads up")
This is important information.
@[/note]
```

### Arguments

| Name  | Type   | Required | Description                      |
| ----- | ------ | -------- | -------------------------------- |
| title | string | no       | Optional heading for the callout |

### Behavior

* Body content is standard Markdown
* Styling depends on callout type (`note`, `tip`, etc.)

### Available types

* `note`
* `tip`
* `info`
* `warning`
* `danger`

---

## `@[include]` — Include another Markdown file

Includes the contents of another Markdown file at this position.

### Syntax

```
@[include](path="partials/intro.md")
```

### Arguments

| Name | Type   | Required | Description                                         |
| ---- | ------ | -------- | --------------------------------------------------- |
| path | string | yes      | Path to Markdown file, relative to the current file |

### Behavior

* Included content is treated as Markdown
* Macros inside included files are expanded
* Recursive includes are prevented automatically
* Paths outside the site source directory are rejected

---

## `@[series]` — Series navigation

Shows navigation and metadata for a post that is part of a series.

### Syntax

```
@[series]
@[series](list=true)
```

### Arguments

| Name  | Type   | Default | Description                  |
| ----- | ------ | ------- | ---------------------------- |
| nav   | bool   | true    | Show previous/next links     |
| list  | bool   | false   | List all parts in the series |
| title | string | none    | Optional title               |

### Behavior

* Only renders if the current post belongs to a series
* Uses the same ordering as series index pages

---

## `@[tags]` — Tags for the current page

Renders the tags assigned to the current page or post.

### Syntax

```
@[tags]
@[tags](style=pills)
```

### Arguments

| Name   | Type   | Default | Description                |
| ------ | ------ | ------- | -------------------------- |
| style  | string | inline  | `inline` or `pills`        |
| prefix | string | none    | Optional label before tags |
| sort   | string | site    | `site` or `alpha`          |

### Example

```
@[tags](style=pills,prefix="Tags:")
```

---

## `@[related]` — Related posts

Shows a list of related posts based on tags and/or series.

### Syntax

```
@[related]
@[related](items=5)
```

### Arguments

| Name  | Type   | Default   | Description                 |
| ----- | ------ | --------- | --------------------------- |
| items | int    | 5         | Number of related posts     |
| by    | string | both      | `tags`, `series`, or `both` |
| title | string | "Related" | Section title               |

### Selection rules

1. Same series (if any)
2. Shared tags (ranked by overlap)
3. Fallback to recent posts

Results are deterministic.

---

## `@[figure]` — Media with caption

Inserts an image or video wrapped in a semantic `<figure>` with caption.

### Syntax

```
@[figure](src="images/diagram.png", caption="System overview")
@[figure](src="video/demo.mp4", caption="Demo clip", maxw="900px")
```

### Arguments

| Name    | Type   | Required | Description                                        |
| ------- | ------ | -------- | -------------------------------------------------- |
| src     | string | yes      | Media path (`images/...` or `video/...`)           |
| caption | string | no       | Caption text                                       |
| alt     | string | no       | Alt/aria label                                     |
| class   | string | no       | Adds `figure-{token}` classes for each token       |
| maxw    | string | no       | Max width (e.g. `900px`, `80%`)                    |
| maxh    | string | no       | Max height (e.g. `480px`)                          |

### HTML semantics

```
<figure class="figure figure-wide">
  <picture>…</picture> / <video>…</video>
  <figcaption>System overview</figcaption>
</figure>
```

---

## `@[kbd]` and `@[key]` — Keyboard keys

Renders keyboard keys or shortcuts.

### Syntax

```
@[kbd]Ctrl[/kbd]
@[kbd]Ctrl+C[/kbd]
@[kbd](text="Ctrl")
@[key]Enter[/key]
```

### Behavior

* Inline macro
* Useful for documentation and tutorials
* Emits `<kbd class="kbd">…</kbd>` for `kbd` and `<kbd class="key">…</kbd>` for `key`

---

## `@[quote]` — Quotation with attribution

Renders a block quote with optional attribution.

### Syntax

```
@[quote](author="Alan Kay", source="Conference talk")
The best way to predict the future is to invent it.
@[/quote]
```

### Arguments

| Name   | Type   | Required | Description       |
| ------ | ------ | -------- | ----------------- |
| author | string | no       | Quote author      |
| source | string | no       | Source or context |
| href   | string | no       | Optional link     |

### HTML semantics

```
<figure class="quote">
  <blockquote class="quote-body">…</blockquote>
  <figcaption class="quote-caption">— Author, <a href="...">Source</a></figcaption>
</figure>
```

---

## Notes & rules

* Macros expand at build time
* Unknown macros are left unchanged
* Macro expansion is depth-limited to prevent infinite recursion
* Macros work inside included files
