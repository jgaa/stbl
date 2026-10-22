[![CI](https://github.com/jgaa/stbl/actions/workflows/ci.yml/badge.svg)](https://github.com/jgaa/stbl/actions/workflows/ci.yml)

# Introduction to stbl

**stbl** is an acronym for *"Static Blog"*.

It all started in 2017 when I wanted a simple website for my freelancing company. I tried several popular options at the time, like Drupal and Jekyll. They required too much effort to get a simple site up and running. WordPress was never an option for me — there are simply too many vulnerabilities.

After a few setbacks, I decided to go with a static website. It’s the easiest and cheapest type of site to host. It’s also the most secure — there is simply no backend to exploit.

I wrote my first static website editor in 1996, and later a commercial CMS based on PHP with a plugin/accelerator written in C++ in the 2010s. I was comfortably familiar with how websites work.

This time I wanted something that provided simple file-based editing (no website editor). I chose Markdown as the source format and designed stbl to translate a collection of Markdown documents into a modern, adaptable website that seamlessly adapts to the browser window — working equally well on mobile and desktop.

Templates are used to generate pages, making it easy to adapt different “skins”.

I wrote the first version in C++ in 2017 and maintained it until 2026. In February 2026, I rewrote the entire application in Rust, rapidly implementing everything on my wishlist.

Although blogs are still an important target for stbl, the current version can also create landing pages, corporate websites, and personal websites.

One important feature of stbl is that it creates fully functional websites without requiring JavaScript. In my opinion, JavaScript is the new Flash. Most security vulnerabilities uncovered today are related to the use and abuse of JavaScript. stbl may use small amounts of JavaScript to enhance functionality when available — but it is never required.

> JavaScript is the new Flash

---

## What it does

stbl reads text files and media files in a special directory structure and generates a static website.

The directory contains special files such as CSS and templates for generating HTML, a configuration file specific to the site, and directories containing one Markdown file per article (post).

---

## Features

* **Responsive static websites** for blogs, landing pages, and general content sites
* **Linux command-line tools** to initialize, validate, plan, build, preview, and publish sites
* **Markdown authoring** with metadata headers, syntax highlighting, macros, and reusable includes
* **Six embedded themes**: stbl, minimal, liberty, paper, mono, and product; color presets, derived palettes, and template/CSS overrides
* **Landing-page layouts** with heroes, feature grids, cards, callouts, and buttons
* **Blog listings**, optional pagination, tag indexes, ordered article series, and RSS feeds
* **Responsive images** with WebP and JPEG/PNG fallbacks, plus AVIF when enabled at compile time
* **Video processing** with scaled MP4 outputs, extracted posters, and optional responsive custom posters
* **Incremental builds** with an optional SQLite cache, parallel execution, and stale output cleanup
* **Deployment assets** with optional content-hashed filenames and gzip/Brotli precompression
* **Search and sharing metadata**: description headers with abstract fallback, canonical URLs, Open Graph tags, sitemap.xml, and robots.txt
* **Configurable URLs**: HTML paths, directory URLs, or directory URLs with migration redirects
* **SVG safety checks** with warning, rejection, and sanitization modes for copied assets
* [Contact and feedback forms](docs/web-forms.md), with an optional CGI receiver using a local mail service
* **Core navigation, reading, native video playback, and forms work without JavaScript**; optional enhancements and third-party comment integrations may use it
* **Template-based comment integrations** using configured provider snippets

---

GPL-3 license.
Free as in speech. Free as in freedom.

---

# How to build

```sh
cargo build --release
```

## Linux binary from GitHub Releases

You can download a prebuilt Linux CLI binary from GitHub Releases:

```sh
curl -fL -o stbl_cli \
  "https://github.com/jgaa/stbl/releases/download/v2.0.0/stbl_cli-linux-x86_64"
chmod +x stbl_cli
sudo mv stbl_cli /usr/local/bin/
```

## Dependencies

stbl uses a number of Rust dependencies. While I generally prefer minimal dependencies, it makes little sense to reinvent well-tested components.

Video processing requires `ffmpeg`. Images are scaled and optimized directly by the application. The optional form receiver additionally needs a CGI-capable web server and a local mail service providing `sendmail`; see [Web Forms](docs/web-forms.md).

---

## Documentation

- [Getting Started](docs/getting-started.md)
- [Configuration Reference (`stbl.yaml`)](docs/configuration.md)
- [CLI Reference](docs/cli.md)
- [Project Structure](docs/project-structure.md)
- [Content Format](docs/content-format.md)
- [Landing Pages](docs/landing-pages.md)
- [Page Types](docs/page-types.md)
- [Timestamps](docs/timestamps.md)
- [Series](docs/series.md)
- [Templates and Themes](docs/templates-and-themes.md)
- [Media](docs/media.md)
- [Web Forms](docs/web-forms.md)
- [Caching](docs/caching.md)
- [FAQ](docs/faq.md)

## Some real websites made with stbl

- [The Last Viking LTD](https://lastviking.eu/)
- [he C++ Freelancer](https://cpp-freelancer.com/)
- [NextApp](https://next-app.org/)
- [DarkSpeak](https://darkspeak.org/)

*Notify me or create a PR if you you use stbl and want your site listed here*

