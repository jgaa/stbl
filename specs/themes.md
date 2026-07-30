# Themes

## 1️ Theme families

### A. Content-First (Low Branding)

Target: blogs, documentation, developers, writers
Tone: neutral, calm, readable

Examples:

* `stbl`
* `minimal`
* `paper`
* `mono`

Characteristics:

* Strong typography
* Narrow content column
* Minimal hero sections
* No heavy gradients
* Subtle accent color
* Dark/light variants

### B. Mission-driven / Technology Movement (Medium Branding)

Target: projects that need to communicate an idea, mission, or movement as much as a product

Examples:

* DarkSpeak
* Tor nodes
* Privacy projects
* Open-source security software
* Decentralized infrastructure
* Digital rights organizations

Characteristics:

* Idea-led narrative and clear principles
* Strong trust and transparency signals
* Technical credibility without corporate polish
* Community, contributor, or participation pathways
* Distinctive but restrained visual identity

### C. Modern Branded (Medium Branding)

Target: consultants, SaaS landing pages, personal brands

Examples:

* `modern`
* `cleanbrand`
* `focus`

Characteristics:

* Hero section with call-to-action
* Larger headings
* Bold accent colors
* Card-based layout
* Section separators
* Soft shadows
* Conversion-focused layout blocks

These are excellent for:

* Sentinelix landing page
* Wellness app
* Indie SaaS

### D. SMB Templates (High Branding)

Target: small businesses that need something that “just works”

Use-cases:

* `lawfirm`
* `accountant`
* `it-services`
* `restaurant`
* `clinic`
* `construction`

Characteristics:

* Structured homepage sections
* Testimonials
* Service grids
* Contact blocks
* Google Maps embed placeholder
* Pricing tables
* “Trust signals”


## 2️ Branding Strength Spectrum

Instead of vague terms, define branding strength like this:

| Level | Description  | Example                      |
| ----- | ------------ | ---------------------------- |
| 1     | Invisible    | pure content                 |
| 2     | Subtle       | accent color + font          |
| 3     | Recognizable | layout personality           |
| 4     | Strong       | strong visual identity       |
| 5     | Dominant     | heavy hero + design language |

Your themes should cover levels 1–4.
Level 5 is usually custom work.


## Standard Themes

These are the themes that are ready or planned.

| Name     | Ready | Family | Strength | Description |
| ---------|-------|--------|----------|-------------|
| stbl     | [x]   | A      | 3        | Default stbl theme |
| minimal  | [x]   | A      | 2        | For blogs |
| paper    | [ ]   | A      | 2        | For blogs |
| mono     | [ ]   | A      | 1        | For blogs |
| liberty  | [x]   | B      | 4        | Mission-driven privacy and open-source technology projects |
| modern   | [ ]   | C      | 4        | Modern blog and/or presentation |
| cleanbrand | [ ] | C      | 4        | Presentation / landing page |


## 3 Custom themes and overrides

All themes supported by stbl is embedded in the app. You can override a theme, or create your own by adding the relevant files in the structure below.

```
site/
  stbl/
    templates/<theme>/*.html
    css/<theme>/*.css
    assets/<theme>/**
    color-presets/*.yaml
```

## The default templates

The following are the default templates used by stbl.

```
crates/stbl_embedded_assets/assets/templates/stbl/templates/partials/intensedebate.html
crates/stbl_embedded_assets/assets/templates/stbl/templates/partials/blog_index.html
crates/stbl_embedded_assets/assets/templates/stbl/templates/partials/footer.html
crates/stbl_embedded_assets/assets/templates/stbl/templates/partials/header.html
crates/stbl_embedded_assets/assets/templates/stbl/templates/partials/list_item.html
crates/stbl_embedded_assets/assets/templates/stbl/templates/disqus.html
crates/stbl_embedded_assets/assets/templates/stbl/templates/base.html
crates/stbl_embedded_assets/assets/templates/stbl/templates/tag_index.html
crates/stbl_embedded_assets/assets/templates/stbl/templates/page.html
crates/stbl_embedded_assets/assets/templates/stbl/templates/series_index.html
```
