Below is a Codex-ready feature spec. I’ve made `landingpage.html` imply `layout: landing` unless explicitly overridden in frontmatter.

# Feature: Landing Page Layout Extensions

## Goal

Extend `stbl` so Markdown can be used to build modern, responsive landing pages with card-based and image/text layouts similar in structure to sites such as Signal and Session.

Markdown must remain the primary source format.

The feature must:

* keep landing page source files readable as Markdown
* avoid embedding HTML or CSS layout details in content
* provide a small set of semantic layout primitives
* allow themes to control the actual appearance and responsive behavior
* work well on desktop, tablet, and mobile
* preserve the existing behavior of normal Markdown pages

This is not intended to become a general-purpose visual page builder.

---

# Layout Selection

Pages may explicitly select the landing-page layout through frontmatter:

```yaml
---
layout: landing
---
```

However, a page rendered using the template:

```text
landingpage.html
```

must default to:

```yaml
layout: landing
```

even when no `layout` value is present in the Markdown frontmatter.

Therefore:

```yaml
---
template: landingpage.html
---
```

is equivalent to:

```yaml
---
template: landingpage.html
layout: landing
---
```

for layout processing purposes.

If `layout` is explicitly present in frontmatter, the explicit value wins.

For example:

```yaml
---
template: landingpage.html
layout: standard
---
```

must not enable the landing-page extensions.

The exact existing naming convention for templates/frontmatter should be respected if `stbl` already uses different field names internally.

---

# Design Principles

The syntax must describe the semantic purpose of content, not its physical layout.

Good:

```markdown
::: features
```

Bad:

```markdown
::: grid columns=3 tablet=2 mobile=1
```

Good:

```markdown
::: section tone=accent
```

Bad:

```markdown
::: section background=#202020 padding=70px
```

The Markdown author describes what something represents.

The theme determines how it looks.

---

# Initial Components

Implement the following components:

* `hero`
* `section`
* `features`
* `card`
* `feature`
* `callout`

Do not introduce arbitrary rows, columns, CSS classes, inline styles, or generic layout containers in this version.

---

# Container Syntax

Use fenced semantic containers:

```markdown
::: component
content
```

````

Containers may have supported attributes:

```markdown
::: feature image="images/example.webp" reverse
...
:::
````

Markdown inside a container must be parsed normally.

Nested containers must be supported where explicitly allowed.

---

# Hero

The `hero` component represents the main introductory section of a landing page.

Example:

```markdown
::: hero
# Private conversations. No surveillance.

DarkSpeak is a private messenger designed around secure peer-to-peer
communication.

[Download](download/) [View source](https://github.com/jgaa/darkspeak)

![DarkSpeak](images/darkspeak.png)
:::
```

A hero may contain normal Markdown including:

* headings
* paragraphs
* images
* links
* buttons represented by links if the theme supports them
* emphasis and other ordinary Markdown

Themes decide the actual layout.

For example, a desktop theme may render text and image side by side, while mobile may stack them.

Supported optional attributes:

```text
align=center
align=left
```

Do not add pixel-based positioning.

---

# Section

The `section` component represents a visual section of the page.

Example:

```markdown
::: section
## Why DarkSpeak?

Content goes here.
:::
```

Sections may optionally specify a semantic tone:

```markdown
::: section tone=subtle
```

Supported tones:

```text
default
subtle
accent
contrast
```

Themes decide what these mean visually.

For example:

* `default` may use the normal page background
* `subtle` may use a lightly differentiated background
* `accent` may use a theme accent background
* `contrast` may use a strong contrasting background

No arbitrary colors or CSS properties should be accepted.

---

# Features

The `features` component represents a collection of equal or roughly equal feature items.

Example:

```markdown
::: features
## Privacy by design

::: card
### No phone number

No phone number or email address is required.
:::

::: card
### Open source

The implementation and protocol can be audited.
:::

::: card
### Private transport

Connections use privacy-preserving transport.
:::
:::
```

`features` may contain:

* introductory Markdown
* multiple `card` containers

Themes determine responsive layout.

Typical behavior may be:

```text
Desktop:  3 cards per row
Tablet:   2 cards per row
Mobile:   1 card per row
```

These values must not be encoded into the Markdown source.

The theme is responsible for determining the best layout.

---

# Card

The `card` component represents a small self-contained item within a feature collection.

Example:

```markdown
::: card
### No tracking

The application does not include advertising trackers.
:::
```

Cards may optionally have an icon:

```markdown
::: card icon="images/icons/privacy.svg"
### No tracking

No advertising or behavioral tracking.
:::
```

The icon is content and is therefore valid component metadata.

Supported attributes:

```text
icon="path"
```

The theme determines:

* card background
* border
* radius
* shadow
* padding
* icon size and placement
* hover effects, if appropriate

None of these should be configurable from Markdown.

A `card` should normally only be valid inside `features`.

---

# Feature

The `feature` component represents a larger section containing descriptive content and usually an image.

Example:

```markdown
::: feature image="images/files.webp"
## Share files privately

Transfer files directly without uploading them to a centralized
cloud service.

[Learn more](docs/files/)
:::
```

Desktop themes may render:

```text
+-----------------------+-----------------------+
|                       | Heading               |
|       Image           |                       |
|                       | Text                  |
|                       |                       |
+-----------------------+-----------------------+
```

Mobile themes should normally stack image and text vertically.

A feature may be reversed:

```markdown
::: feature image="images/profile.webp" reverse
## You control your identity

Your identity is based on cryptographic keys.
:::
```

On desktop this may swap image and text positions.

On narrow screens, the theme may ignore `reverse` and use the normal stacked presentation.

Supported attributes:

```text
image="path"
reverse
```

Do not introduce explicit percentages, column sizes, or breakpoints.

---

# Callout

The `callout` component represents a prominent statement or call to action.

Example:

```markdown
::: callout
## Ready to try DarkSpeak?

[Download DarkSpeak](download/)
:::
```

Callouts may contain normal Markdown.

The theme controls visual emphasis.

Typical uses include:

* download CTA
* signup CTA
* strong product statement
* final page section
* important project message

---

# Markdown Compatibility

All normal Markdown must continue to work outside these containers.

For example:

```markdown
# Normal page heading

This remains normal Markdown.

::: feature image="feature.webp"
## Enhanced section

This uses the landing-page extension.
:::

More ordinary Markdown.
```

Landing-page processing must not require every element on the page to use a custom component.

---

# Landing Components Outside Landing Layout

The semantic containers should only receive special landing-page rendering when the page uses:

```yaml
layout: landing
```

or implicitly receives it from `landingpage.html`.

For non-landing pages, choose one of these implementation approaches:

1. Treat unknown landing containers as transparent containers and render their Markdown children normally.
2. Ignore their special layout semantics but preserve the contained content.

Do not silently discard content.

Prefer graceful degradation.

---

# Theme Integration

Landing components must be rendered by the active theme.

The precise implementation should fit the current `stbl` theme architecture.

Conceptually, themes should be able to provide rendering for:

```text
hero
section
features
card
feature
callout
```

For example:

```text
themes/<theme>/
    ...
    landing/
        hero.html
        section.html
        features.html
        card.html
        feature.html
        callout.html
```

This directory layout is illustrative only.

Do not introduce a new theme architecture if the existing template system can support the components cleanly.

---

# Responsive Behavior

Responsive layout belongs entirely to the theme.

The Markdown source must not specify:

* breakpoints
* screen widths
* card counts per row
* pixel widths
* margins
* padding
* flexbox configuration
* CSS grid configuration

Themes should provide sensible behavior such as:

```text
wide screen:
    cards arranged horizontally
    feature image/text shown side by side

small screen:
    cards stacked vertically
    feature image/text stacked vertically
```

The generated page must not require horizontal scrolling at normal mobile viewport sizes.

Images must scale responsively.

---

# Accessibility

Generated landing-page components must preserve normal HTML accessibility.

Requirements include:

* headings must remain real heading elements
* links must remain real links
* images must preserve Markdown alt text
* decorative images/icons should not unnecessarily appear to assistive technology
* layout must not depend on DOM ordering that makes the page confusing for screen readers
* `reverse` should preferably be implemented visually without producing illogical reading order

Responsive presentation must not change the semantic content order.

---

# Parsing

The landing-page syntax should be parsed into semantic nodes rather than transformed directly into arbitrary HTML during Markdown parsing.

Conceptually:

```text
LandingPage
 ├── Hero
 │    └── Markdown
 ├── Section
 │    └── Markdown
 ├── Features
 │    ├── Markdown
 │    ├── Card
 │    │    └── Markdown
 │    └── Card
 │         └── Markdown
 ├── Feature
 │    ├── image
 │    ├── reverse
 │    └── Markdown
 └── Callout
      └── Markdown
```

The actual internal representation should fit the existing parser and renderer architecture.

Do not create a parallel Markdown implementation if the current Markdown parser can be extended cleanly.

---

# Attribute Parsing

Attributes should support:

```markdown
::: feature image="images/foo.webp" reverse
```

Supported forms should remain deliberately simple.

Suggested grammar:

```text
name
name=value
name="value containing spaces"
```

Boolean attributes such as:

```text
reverse
```

should evaluate to true when present.

Unknown attributes should preferably produce a useful warning during generation rather than silently changing rendering behavior.

Malformed component syntax should produce a useful error containing the source file and line where possible.

---

# Validation

Validate component nesting.

Examples:

Valid:

```markdown
::: features
::: card
...
:::
:::
```

Invalid or at least warning-worthy:

```markdown
::: card
...
:::
```

outside `features`.

Do not make the parser excessively strict where graceful behavior is harmless, but detect obvious mistakes.

---

# Theme Fallback

A landing page must remain usable even if a theme does not implement specialized rendering for every component.

Fallback behavior should render the content vertically using simple semantic HTML.

For example:

```text
hero       -> section/div containing Markdown
features   -> section containing children
card       -> article/div containing Markdown
feature    -> section containing image + Markdown
callout    -> aside/section containing Markdown
```

This prevents content from disappearing when:

* switching themes
* using older themes
* developing a new theme
* previewing incomplete themes

---

# Example Complete Landing Page

```markdown
---
title: DarkSpeak
template: landingpage.html
---

::: hero
# Private conversations. No surveillance.

DarkSpeak is a secure peer-to-peer messenger designed for private
communication.

[Download](download/) [Source code](https://github.com/jgaa/darkspeak)

![DarkSpeak application](images/main-window.webp)
:::

::: section tone=subtle
::: features
## Privacy by design

::: card icon="images/icons/identity.svg"
### No phone number

Your identity is based on cryptographic keys.
:::

::: card icon="images/icons/server.svg"
### No central message server

Messages are exchanged directly between peers.
:::

::: card icon="images/icons/source.svg"
### Open source

The application and protocol can be inspected and audited.
:::
:::
:::

::: feature image="images/chat.webp"
## Private messaging

Communicate without handing your conversations to a centralized
messaging service.
:::

::: feature image="images/files.webp" reverse
## Secure file sharing

Exchange files directly between trusted contacts.
:::

::: section tone=contrast
::: callout
## Take control of your conversations.

[Download DarkSpeak](download/)
:::
:::
```

Because the template is `landingpage.html`, this page automatically behaves as though it contained:

```yaml
layout: landing
```

---

# Non-Goals

Do not implement the following as part of this feature:

* arbitrary HTML page-builder functionality
* drag-and-drop editing
* Bootstrap-like rows and columns
* user-defined CSS classes
* inline styles
* arbitrary colors
* pixel dimensions
* responsive breakpoints in Markdown
* JavaScript-based layout
* animations
* carousels
* sliders
* tabs
* forms
* testimonials-specific components
* pricing-table-specific components
* statistics-specific components
* generic grid components

Additional semantic components can be added later when there is a concrete use case.

---

# Security

The extensions must not weaken the security properties of the existing Markdown renderer.

---

# Tests

Add tests covering at least:

## Layout detection

* explicit `layout: landing`
* `landingpage.html` with no explicit layout
* `landingpage.html` with explicit non-landing layout override
* normal template without landing layout

## Parsing

* each component individually
* component attributes
* quoted attributes
* boolean `reverse`
* nested `features` + `card`
* normal Markdown inside components
* multiple components in one document

## Invalid input

* unclosed component
* malformed attributes
* unsupported component
* invalid nesting
* unsupported attribute

## Rendering

* specialized theme rendering
* theme fallback rendering
* escaping
* image alt text
* links
* headings

## Regression

Existing Markdown pages and articles must render exactly as before unless they opt into the landing layout.

---

# Acceptance Criteria

The feature is complete when:

1. A Markdown page can use the six semantic landing components.
2. Normal Markdown remains valid inside those components.
3. `layout: landing` activates landing-page rendering.
4. `landingpage.html` automatically implies `layout: landing`.
5. An explicitly specified layout overrides that default.
6. Themes control responsive layout and appearance.
7. No responsive or CSS layout details are required in Markdown.
8. Feature cards adapt cleanly from multi-column desktop layout to stacked mobile layout.
9. Image/text feature sections adapt cleanly to mobile.
10. Themes without specialized landing support still render all content.
11. Existing non-landing Markdown pages remain backward compatible.
12. Component attributes cannot inject arbitrary HTML, CSS, or JavaScript.
13. Parser and renderer tests cover the new syntax and default-layout behavior.
    :::

One implementation choice I’d preserve strongly during the Codex work is **semantic AST/node handling rather than expanding these directly to HTML during Markdown parsing**. That will make adding another component later—say `stats` or `downloads`—much less likely to turn into parser/template spaghetti.
