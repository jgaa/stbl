## Config format

* YAML (`stbl.yaml`)
* Loaded once per site
* Fully validated before content processing

---

## Required fields

```yaml
site:
  id: string
  title: string
  base_url: string
  language: string
```

Missing required fields are **errors**.

### site.url_style

Controls how URLs and output paths are generated.

Allowed values:

- `html` (default)
  - Pages are generated as `/path/name.html`

- `pretty`
  - Pages are generated as `/path/name/index.html`
  - Links use `/path/name/`

- `pretty_with_fallback`
  - Same as `pretty`
  - Additionally generates `/path/name.html` as a redirect page
  - Intended for migrating existing sites without breaking links


## Full schema (MVP)

```yaml
site:
  id: string
  title: string
  abstract: string
  base_url: string
  language: string
  timezone: string   # optional
  url_style: html   # default, safe for existing sites

banner:
  widths: [int]
  quality: int
  align: int

menu:
  - title: string
    href: string

people:
  default: string
  entries:
    <id>:
      name: string
      email: string
      links:
        - id: string
          name: string
          url: string
          icon: string

system:
  date:
    format: string
    roundup_seconds: int

publish:
  command: string

rss:
  enabled: bool
  max_items: int
  ttl_days: int   # optional

seo:
  sitemap:
    priority:
      frontpage: int
      article: int
      series: int
      tag: int
      tags: int

comments: map   # parsed but not enforced in MVP
chroma: map
plyr: map
```

---

## RSS rules

* RSS is generated only if `rss.enabled: true`
* If enabled:

  * Missing required RSS fields → **error**
* Filtering:

  * Apply `ttl_days` cutoff first (if present)
  * Then apply `max_items`

---

## Example `stbl.yaml`

```yaml
site:
  id: "lastviking-eu"          # mandatory and stable
  title: "The Last Viking LTD" # required
  abstract: "Software & Coffee at the Edge of the Universe"
  base_url: "https://lastviking.eu/"  # required
  language: "en"               # required
  # timezone: "Europe/Sofia"   # optional; if omitted use local timezone

banner:
  widths: [94, 128, 248, 360, 480, 640, 720, 950, 1280, 1440, 1680, 1920, 2560]
  quality: 95
  align: 0

menu:
  - title: "Home"
    href: "./"
  - title: "Blog"
    href: "./blog.html"
  - title: "Freelancing"
    href: "https://cpp-freelancer.com/"
  - title: "Github"
    href: "https://github.com/jgaa"
  - title: "Contact"
    href: "./contact.html"
  - title: "About"
    href: "./about.html"

people:
  default: "jgaa"
  entries:
    jgaa:
      name: "Jarle Aase"
      email: "jgaa@jgaa.com"
      links:
        - id: "e-mail"
          name: "jgaa"
          url: "mailto:contact@lastviking.eu"
          icon: "{{rel}}artifacts/feather/mail.svg"
        - id: "github"
          name: "jgaa"
          url: "https://github.com/jgaa"
          icon: "{{rel}}artifacts/feather/github.svg"
        - id: "linkedin"
          name: "Jarle Aase"
          url: "https://www.linkedin.com/in/jgaa-from-north"
          icon: "{{rel}}artifacts/li123.svg"

system:
  date:
    format: "%A %B %e, %Y"
    roundup_seconds: 1800

publish:
  command: "rsync -a --delete {{local-site}}/ {{destination}}/"

rss:
  enabled: true
  max_items: 16
  ttl_minutes: 1800
  # ttl_days: 30   # optional additional cutoff you requested

seo:
  sitemap:
    priority:
      frontpage: 100
      article: 90
      series: 95
      tag: 40
      tags: 80

comments:
  # default: "disqus"
  # disqus:
  #   src: "https://the-last-viking.disqus.com/embed.js"
  #   template: "disqus.html"

chroma:
  enabled: "auto"  # true|false|auto
  style: "friendly"
  # path: "/usr/local/bin/chroma"

plyr:
  js: "https://cdn.plyr.io/3.7.8/plyr.js"
  css: "https://cdn.plyr.io/3.7.8/plyr.css"
  default: 480

```
