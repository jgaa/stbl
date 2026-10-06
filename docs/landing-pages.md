# Landing Pages

Landing pages use ordinary Markdown plus a small set of semantic containers.
They are intended for product, project, and organization pages while keeping
the source readable and independent of a particular visual design.

## Enable the landing layout

Use `layout: landing` in the page header:

```markdown
---
title: Example Product
layout: landing
---
```

`template: landingpage.html` also enables the landing layout when no `layout`
value is present. An explicit layout wins, so `layout: standard` keeps the page
as normal Markdown even with that template.

## Components

Containers use `::: name` and a closing `:::`. Markdown inside a component is
rendered normally, including headings, links, images, lists, and emphasis.

```markdown
::: hero align=center
# Private conversations. No surveillance.

DarkSpeak is a private messenger designed around peer-to-peer communication.

[Download](download/)
:::
```

The available components are:

- `hero` — the introductory page section; accepts `align=left` or `align=center`,
  and the boolean `reverse` attribute. By default, the first standalone Markdown
  image sits to the right of the text on desktop. `reverse` puts it on the left.
  Both stack with text first on mobile. `align=center` keeps the centered,
  stacked presentation.
- `section` — a visual section; accepts `tone=default`, `subtle`, `accent`, or
  `contrast`.
- `features` — a collection of feature cards.
- `card` — a small feature item, normally placed inside `features`; accepts
  `icon="path"`.
- `feature` — a larger image-and-text section; accepts `image="path"` and the
  boolean `reverse` attribute.
- `callout` — a prominent statement or call to action.

Attributes are deliberately limited to semantic content. Do not use columns,
breakpoints, colors, inline styles, or CSS classes in page content.

## Hero images

Use a standalone Markdown image without width constraints; the theme sizes it
within the available image area:

```markdown
::: hero
# I built NextApp to organize myself

NextApp is a private personal organizer for your work and your life.

![NextApp application](images/nextapp-banner-blue.png)
:::
```

Use `::: hero reverse` for an image on the left. To change the desktop split,
use an image constraint such as `;maxw=60%`: the image receives 60% of the
available hero width after the gap, and the text receives the rest. Percentage
constraints are applied once to the column; mobile images fill the stacked
image area. Without a percentage, the desktop split is equal. Inline images, images in lists
or quotes, and additional images retain ordinary Markdown rendering.

Split hero images use the configured image variants, with `srcset` and `sizes`
so the browser can select a width for the image area and device pixel density.
All configured widths up to the source width are generated; images are never
upscaled. Hero images load eagerly because they appear at the top of the page.

## Buttons without JavaScript

The `product` theme styles `@[button]` macros as ordinary accessible links:

```markdown
::: hero
# A better way to get things done

Stay organized, reduce stress and focus on what matters.

@[button](text="Get Started", href="download")
@[button](text="Watch Video", href="#demo", kind=secondary, icon=play)

![Application](images/app.png;maxw=60%)
:::

::: callout
## Ready to get started?

@[button](text="Get Started", href="download")
:::
```

`text` and `href` are required. `kind` accepts `primary` (default) or `secondary`.
The optional `icon=play` adds a CSS play symbol; the visible text remains the
accessible name. Relative page destinations follow the site's URL style and
base path; a fragment can target a video section on the same page. Other themes
preserve the clickable link even when they do not style buttons. Macros must be
enabled (the default). Unsupported arguments and unsafe URL schemes leave the
macro unexpanded.

Configure `theme.header.action` to repeat the primary action beside the logo
and menu. Buttons keep their color scheme's accent, with a contrasting text
color generated at build time.

## Complete example

```markdown
---
title: DarkSpeak
template: landingpage.html
---

::: hero
# Private conversations. No surveillance.

![DarkSpeak application](images/main-window.webp)
:::

::: section tone=subtle
::: features
## Privacy by design

::: card icon="images/icons/identity.svg"
### No phone number

Your identity is based on cryptographic keys.
:::

::: card
### Open source

The application and protocol can be inspected and audited.
:::
:::
:::

::: feature image="images/files.webp" reverse
## Secure file sharing

Exchange files directly between trusted contacts.
:::

::: callout
## Ready to try DarkSpeak?

[Download DarkSpeak](download/)
:::
```

## Rendering and validation

Themes control the responsive presentation. The built-in themes stack cards
and image/text features on small screens, use a card grid on desktop, and keep
the source reading order when visually reversing a feature.

Outside the landing layout, recognized container fences are transparent: their
Markdown content is rendered normally. This lets content remain readable when
a page changes layouts or is viewed with an older theme.

Malformed containers are build errors. Unsupported attributes and a `card`
outside `features` produce warnings so content is not silently lost.
