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

* Creates **responsive HTML5 websites**
* **Command-line program** (for Linux)
* **Markdown-based writing**
* **Syntax-colored source code snippets**
* **Scales media files** — images and videos are optimized for various screen sizes
* **Fast rebuilds** — uses caching and only regenerates changed content (like *make*)
* **Security features**, including scanning SVGs for hidden tracking
* **Color schemes** — easily change or modify colors
* **Template-driven rendering** for maximum flexibility
* **Macros** for dynamic content injection
* Works fully **without JavaScript**
* [SEO friendly](https://lastviking.eu/stbl_and_seo.html)
* Easy to add [commenting with Disqus](https://lastviking.eu/stbl_with_disqus.html)
* Easy to add [commenting with IntenseDebate](https://lastviking.eu/stbl_with_intensedebate.html)

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

The only runtime dependency is `ffmpeg`. This is required only if you use videos in your content. Images are scaled and optimized directly by the application.

---

## Documentation

- [Getting Started](docs/getting-started.md)
- [CLI Reference](docs/cli.md)
- [Project Structure](docs/project-structure.md)
- [Content Format](docs/content-format.md)
- [Page Types](docs/page-types.md)
- [Timestamps](docs/timestamps.md)
- [Series](docs/series.md)
- [Templates and Themes](docs/templates-and-themes.md)
- [Media](docs/media.md)
- [Caching](docs/caching.md)
- [FAQ](docs/faq.md)

## Some real websites made with stbl

- [The Last Viking LTD](https://lastviking.eu/)
