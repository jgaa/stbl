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
* SVG safety checks for embedded assets (scan, warn/fail, or sanitize)

---

GPL-3 license.
Free as in speech. Free as in freedom.

---

# How to build

```sh
cargo build --release
```

## Dependencies

stbl uses a number of Rust dependencies. While I generally prefer minimal dependencies, it makes little sense to reinvent well-tested components.

The only runtime dependency is `ffmpeg`. This is required only if you use videos in your content. Images are scaled and optimized directly by the application.

---

## Quickstart

```sh
mkdir myblog
cd myblog
stbl_cli init
```

Edit `stbl.yaml` and set at least a site name and tagline. Then go to `./articles` and create some content. An `index.md` file is required for the front page.

Each article begins with a header enclosed in three dashes, followed by Markdown content.

Example:

```markdown
---
title: Getting Things Done
abstract: Discovering GTD: The Art of Getting Things Done
tags: GTD, David Allen
published: 2025-05-31 15:09
---

## What is GTD?

...
```

To preview your site locally:

```
stbl_cli build --preview-open
```

This generates the site in:

```
~/.cache/stbl/<site-name>/out
```

An embedded web server is started on localhost, and your browser will open the front page automatically.

In `stbl.yaml`, you can specify a command to publish the site. With that enabled, for example using `rsync`, you can generate and publish your site like this:

```sh

stbl_cli build --publish-to example.com:/var/www/yoursite

```


