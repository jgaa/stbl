# `minimal` theme specification

The notes define `minimal` as a **content-first blog theme**, with branding strength **2: subtle**. Its identity should come from typography, spacing, and a restrained accent colour—not from decorative layouts, cards, gradients, or prominent branding. 

The most important distinction should be:

* `minimal`: clean contemporary publishing
* `paper`: warmer, more editorial and tactile
* `mono`: nearly invisible, technical, monochromatic
* `stbl`: more recognisable layout personality

## Overall character

`minimal` should feel like a carefully typeset personal blog.

It should be:

* calm
* modern
* spacious without wasting screen space
* readable on phones and large monitors
* suitable for technical writing, essays, project updates, and ordinary blog posts
* clearly designed, but not visibly “themed”

The content should always dominate. The reader should notice that the site is pleasant to read, not spend time noticing the design.

## Shared layout

### Page width

Use two related width constraints:

```css
--layout-width: 72rem;
--content-width: 46rem;
```

The header and footer may use the wider `72rem` container. Article text and the primary front-page feed should normally use the narrower `46rem` width.

Suggested horizontal padding:

```css
--page-padding: clamp(1.1rem, 4vw, 2rem);
```

This gives approximately:

* 17–18 px on narrow phones
* 24–32 px on tablets and desktops

### Vertical rhythm

Use generous but consistent spacing rather than oversized empty regions.

```css
--space-xs: 0.4rem;
--space-sm: 0.75rem;
--space-md: 1.25rem;
--space-lg: 2rem;
--space-xl: 3.25rem;
--space-2xl: 5rem;
```

The header should not consume much vertical space. The first meaningful content should appear quickly.

### Typography

Use the system font stack by default:

```css
font-family:
    system-ui,
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    sans-serif;
```

That keeps the theme dependency-free, fast, native-looking, and neutral.

Body text:

```css
font-size: clamp(1rem, 0.97rem + 0.15vw, 1.075rem);
line-height: 1.7;
```

Article headings:

```css
h1: clamp(2rem, 5vw, 3.2rem);
h2: clamp(1.45rem, 3vw, 2rem);
h3: clamp(1.2rem, 2vw, 1.5rem);
```

Headings should use moderate weight rather than extreme boldness:

```css
font-weight: 650;
letter-spacing: -0.02em;
```

Body text should generally use weight `400`. Metadata can use a slightly smaller font and muted colour.

Do not use uppercase section headings except perhaps for very small accessibility labels. Uppercase navigation or metadata would make the design feel more branded than intended.

## Colour system

The theme should be driven by a small semantic palette rather than many component-specific colours.

### Light mode

```css
--background: #ffffff;
--surface: #f7f7f6;
--text: #202020;
--text-muted: #6b6b68;
--border: #ddddda;
--accent: #315f8c;
--accent-hover: #234d77;
--code-background: #f3f3f1;
```

### Dark mode

```css
--background: #171817;
--surface: #202220;
--text: #e8e9e6;
--text-muted: #a5a8a2;
--border: #393c38;
--accent: #8ab4de;
--accent-hover: #afd0ed;
--code-background: #202220;
```

The actual accent colour should be replaceable through a colour preset. Everything else should be derived from stable neutral colours.

Avoid:

* gradients
* tinted page backgrounds
* accent-coloured headings
* large blocks using the accent colour
* shadows except where browser-native controls genuinely need separation

The accent should mainly appear on links, focus indicators, and very small interface details.

## Header

The header should be shared by the front page and articles.

### Desktop structure

```text
Site name                         About  Archive  RSS
```

The site name appears on the left. Navigation appears on the right.

The header should:

* use the wider layout container
* have a minimum height of approximately `4rem`
* use ordinary text rather than a logo treatment by default
* optionally support a small site icon without assuming one exists
* have either no bottom border or a very subtle one
* remain static rather than sticky

The site name should be around `1.05rem`, weight `650`. It should not be an oversized brand mark.

Navigation links should be around `0.95rem`, with comfortable spacing.

### Mobile structure

On narrow screens:

```text
Site name                    Menu
```

The menu may expand below the header. Do not reduce links to an ambiguous icon without an accessible label.

For sites with two or three short navigation entries, wrapping the links onto a second line may be preferable to adding JavaScript.

## Front page

The front page should look like a publication index, not a landing page.

### Introductory area

The page may begin with:

```text
Site title

A concise description of the site, its author, or its subject.
```

This is not a hero section. It should have no background panel, illustration, button group, or oversized typography.

Recommended limits:

* title: one line where possible
* description: one or two short paragraphs
* maximum text width: approximately `38rem`
* bottom spacing: `3rem`

The site title should normally be between `2.25rem` and `3rem`, depending on viewport width.

When no separate front-page introduction is configured, the article list should begin directly beneath the header.

### Article list

The default list should be chronological and linear.

```text
Article title
April 14, 2026 · 8 min read

A short summary or the opening excerpt of the article.

tags
────────────────────────────────────
```

Each item should contain:

1. article title
2. publication date
3. optional reading time
4. optional summary
5. optional tags
6. subtle separator

The title is the strongest visual element. It should not appear inside a card.

Suggested sizing:

```css
.article-list-title {
    font-size: clamp(1.35rem, 2.5vw, 1.75rem);
    line-height: 1.25;
    font-weight: 650;
}
```

Article metadata:

```css
font-size: 0.875rem;
color: var(--text-muted);
```

Summary:

```css
line-height: 1.6;
color: var(--text);
```

Each article item should have approximately `2rem` vertical padding. Separators should use `--border`.

Do not use:

* card backgrounds
* rounded rectangles around every post
* thumbnail grids
* “Read more” buttons for every entry
* alternating layouts
* decorative category labels

The article title itself is the primary link. A small textual “Continue reading” link may be supported, but it should be optional and visually secondary.

### Featured images

Featured images should be optional.

When present, they should appear:

* above the article title, or
* below the metadata and above the summary

For `minimal`, I would place them **above the title** on the front page. This preserves a predictable reading order and avoids a two-column magazine layout.

Images should:

* use the full width of the content column
* retain their natural aspect ratio
* have a very small radius, around `3px`, or no radius
* have no shadow
* use `loading="lazy"` below the first visible item

A feed containing no images should look completely intentional.

### Pagination

Use simple previous and next navigation:

```text
← Newer posts                         Older posts →
```

Do not use a row of numbered buttons unless stbl already exposes numbered pagination and it would be cumbersome to replace.

## Article page

The article page should have a single uninterrupted reading column.

### Article header

Recommended structure:

```text
Article title

April 14, 2026 · 8 min read
Tags: development, security
```

Then, where present:

```text
Introductory summary or lead paragraph
```

The article title should not exceed the normal content width. Do not make it span the wider site container.

Suggested spacing:

* header to title: `3rem` on desktop, `2rem` on mobile
* title to metadata: `1rem`
* metadata to article body: `2.5rem`
* article end to related navigation: `3rem`

An optional article image may appear after the metadata and before the body. It should use the content width by default. The implementation may later support a “wide” image class, but the theme should not assume that every post needs one.

### Body content

The body should use approximately `46rem` as its maximum width.

Paragraph spacing:

```css
article p {
    margin-block: 0 1.35em;
}
```

Headings should have noticeably more space above than below:

```css
article h2 {
    margin-block: 2.3em 0.7em;
}

article h3 {
    margin-block: 1.8em 0.6em;
}
```

This helps headings group with the text they introduce.

### Links

Links in article content should be identifiable without relying solely on colour.

Recommended:

```css
article a {
    color: var(--accent);
    text-decoration: underline;
    text-decoration-thickness: 0.08em;
    text-underline-offset: 0.15em;
}
```

Navigation and article-title links may omit the underline until hover because their purpose is already apparent from their placement.

### Lists

Lists should remain conventional and readable.

* normal bullets or decimal numbering
* approximately `1.5em` left indentation
* `0.4em` spacing between longer list items
* no custom decorative bullets

### Blockquotes

Blockquotes should be restrained:

```css
border-inline-start: 3px solid var(--border);
padding-inline-start: 1.25rem;
color: var(--text-muted);
```

No large quotation marks, accent-coloured panels, or italicisation of the entire block by default.

### Code

Inline code:

* subtle neutral background
* small radius
* slightly reduced size
* enough padding to remain legible

Code blocks:

* horizontally scrollable
* neutral surface colour
* no fake window controls
* no heavy border
* no forced syntax theme where syntax highlighting is unavailable
* preserve visible focus when keyboard-scrolled

Suggested:

```css
pre {
    padding: 1.25rem;
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--code-background);
    line-height: 1.55;
}
```

The code font can also use the platform stack:

```css
ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", monospace
```

### Tables

Tables should remain plain:

* full width where practical
* horizontal scrolling on small screens
* left-aligned text
* subtle horizontal borders
* no zebra striping by default
* slightly stronger header weight

### Figures and captions

Figures should have approximately `2rem` vertical margin. Captions should be smaller, muted, and centred or left-aligned consistently. I recommend left alignment because it fits the understated character.

### Article footer

At the end of the article:

```text
Tags: qt, security, development

← Previous article                 Next article →
```

Then optional comments.

The article footer should be separated from the article body by whitespace, not by a large panel.

Series information, when present, may appear in a subtle bordered block:

```text
Part 2 of Building Shared

Previous: Defining the threat model
Next: Device enrolment
```

This is one of the few places where a `surface` background is useful because the information is structurally distinct.

## Comments

Disqus and IntenseDebate should appear after the article footer and navigation.

Comments should inherit the normal content width. The theme should provide a clear heading such as `Comments`, but avoid wrapping the embedded comment system in a decorative card.

A large vertical gap before comments is appropriate because they represent a change from authored content to discussion.

## Tag and series pages

These should reuse the same article list presentation as the front page.

Suggested heading:

```text
Articles tagged “security”
```

or:

```text
Series: Building Shared
```

A series description can appear below the heading. Do not introduce a new grid or card system for these pages.

## Footer

The footer should be compact and use the wider layout container.

Possible structure:

```text
© 2026 Author name                  RSS · Source · Privacy
```

On mobile, this may stack into two lines.

The footer should have:

* a subtle top border
* muted text
* approximately `2.5rem` top and bottom padding
* no multi-column sitemap
* no newsletter form by default
* no large branding repetition

## Responsive behaviour

### Narrow phones

Below approximately `40rem`:

* use the full available width with page padding
* reduce heading sizes through `clamp()`
* allow metadata to wrap
* stack article navigation vertically when titles are long
* make tables and code blocks horizontally scrollable
* do not reduce body text below `1rem`

### Tablets and desktops

The content column should remain narrow rather than expanding with the screen. The surrounding whitespace is part of the theme.

### Very large screens

Do not increase the article body width beyond approximately `46rem`. Text lines should generally remain around 65–75 characters.

## Dark mode

Support all three situations:

1. automatic mode through `prefers-color-scheme`
2. explicitly selected light mode
3. explicitly selected dark mode

The mechanism should permit a site or user setting to override the system preference.

Dark mode should not be pure black. Images should remain unchanged; applying global opacity or filters would distort authored content.

The theme should declare:

```css
color-scheme: light dark;
```

Native controls should therefore follow the selected mode.

## Accessibility requirements

The initial implementation should include:

* visible keyboard focus using the accent colour
* a skip-to-content link
* semantic `header`, `nav`, `main`, `article`, and `footer` elements
* sufficient contrast in both modes
* links in body text distinguished by underline as well as colour
* no information represented only through colour
* support for `prefers-reduced-motion`
* descriptive menu labels
* logical heading order
* minimum comfortable pointer targets in navigation

There is little reason for the theme to contain significant animation. A short colour transition on links is enough, and even that can be disabled under reduced-motion preferences.

## Motion

Use almost none.

Permitted:

```css
transition: color 120ms ease;
```

Avoid:

* content entering animations
* fading article cards
* animated underlines
* parallax effects
* sticky-header movement
* image zoom on hover

## Template mapping

The existing default template set provides a sensible starting point for implementing the theme. 

| Template                      | `minimal` responsibility                       |
| ----------------------------- | ---------------------------------------------- |
| `base.html`                   | Document structure, metadata, colour mode, CSS |
| `partials/header.html`        | Compact site identity and navigation           |
| `partials/footer.html`        | Copyright and secondary links                  |
| `partials/blog_index.html`    | Introductory text and chronological feed       |
| `partials/list_item.html`     | Linear article entry                           |
| `page.html`                   | Article or ordinary page layout                |
| `tag_index.html`              | Heading plus reused article list               |
| `series_index.html`           | Series introduction plus ordered article list  |
| `disqus.html`                 | Comments integration spacing                   |
| `partials/intensedebate.html` | Comments integration spacing                   |

The main reusable design unit should be `list_item.html`. The front page, tags, archives, and series should all use essentially the same article-list item rather than developing separate visual systems.

## Explicit non-goals

The first version of `minimal` should not include:

* a full-screen hero
* sidebars
* card grids
* floating navigation
* author profile cards
* social-sharing button clusters
* newsletter panels
* related-article thumbnails
* decorative backgrounds
* gradients
* prominent shadows
* multiple accent colours
* JavaScript-dependent layout behaviour

## Recommended front-page wireframe

```text
┌─────────────────────────────────────────────────────────────┐
│ Site name                              About  Archive  RSS   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ Site title                                                  │
│ A concise description of the site and what is published     │
│ here.                                                       │
│                                                             │
│ Article title                                               │
│ July 28, 2026 · 7 min read                                  │
│                                                             │
│ A short article summary that can wrap across two or three   │
│ lines without becoming a separate visual component.         │
│ development · security                                      │
│                                                             │
│ ─────────────────────────────────────────────────────────── │
│                                                             │
│ Another article title                                       │
│ July 20, 2026 · 5 min read                                  │
│                                                             │
│ Another short summary.                                      │
│                                                             │
│ ─────────────────────────────────────────────────────────── │
│                                                             │
│ ← Newer posts                              Older posts →     │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│ © 2026 Author                              RSS · Source      │
└─────────────────────────────────────────────────────────────┘
```

## Recommended article wireframe

```text
┌─────────────────────────────────────────────────────────────┐
│ Site name                              About  Archive  RSS   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ Article title that may wrap naturally                       │
│ across more than one line                                   │
│                                                             │
│ July 28, 2026 · 7 min read                                  │
│ development · security                                      │
│                                                             │
│ Optional lead paragraph with slightly stronger presence.    │
│                                                             │
│ Ordinary article content begins here. The column remains     │
│ narrow even when the browser window is very wide.            │
│                                                             │
│ Section heading                                             │
│                                                             │
│ More content, images, code, tables, or blockquotes.          │
│                                                             │
│ Tags: development, security                                 │
│                                                             │
│ ← Previous article                       Next article →      │
│                                                             │
│ Comments                                                    │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│ © 2026 Author                              RSS · Source      │
└─────────────────────────────────────────────────────────────┘
```

The core design decision is that **`minimal` is a single-column publishing theme with one quiet accent colour and no component decoration unless structure genuinely requires it**. That gives it a clear identity while leaving enough visual territory for `paper`, `mono`, and `stbl` to remain meaningfully different.
