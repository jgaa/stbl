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

- `hero` — the introductory page section; accepts `align=left` or `align=center`.
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
