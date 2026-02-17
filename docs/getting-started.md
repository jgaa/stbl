# Getting Started

This guide gets you from an empty directory to a locally previewed `stbl` site.

## Prerequisites

- Linux shell environment
- Rust toolchain (`cargo`, `rustc`)
- `stbl_cli` available either:
  - from this repo via `cargo run -p stbl_cli -- ...`, or
  - as an installed binary from GitHub Releases
- Optional: `ffmpeg` (only required if you use video media)

## 1. Install `stbl_cli`

Option A, download the Linux binary from GitHub Releases:

```sh
curl -fL -o stbl_cli \
  "https://github.com/jgaa/stbl/releases/download/v2.0.0/stbl_cli-linux-x86_64"
chmod +x stbl_cli
sudo mv stbl_cli /usr/local/bin/
```

Option B, build from source.

## 2. Build `stbl_cli`

From the repository root:

```sh
cargo build -p stbl_cli
```

If you prefer running without installing, use:

```sh
cargo run -p stbl_cli -- --help
```

## 3. Create a New Site

Create and enter a new project directory:

```sh
mkdir myblog
cd myblog
```

Initialize site files:

```sh
stbl_cli init
```

If you are running from the source repo instead of an installed binary:

```sh
cargo run -p stbl_cli -- init
```

After init, you should have at least:

- `stbl.yaml`
- `articles/`

## 4. Configure the Site

Open `stbl.yaml` and set minimum identity fields such as:

- site name
- tagline
- base URL (when relevant for feeds/sitemaps/publishing)

Keep this file under version control.

## 5. Add Your Front Page

Create `articles/index.md` with a header and content:

```markdown
---
title: Home
---

Welcome to my site.
```

The root `index.md` is a cover page.

## 6. Add One Article

Create `articles/my-first-post.md`:

```markdown
---
title: My First Post
abstract: First test post
tags: intro, notes
published: 2026-02-17 10:00
---

Hello from stbl.
```

## 7. Build and Preview

Run:

```sh
stbl_cli build --preview-open
```

This will:

- generate site output,
- start a local preview server,
- open a browser (when supported by your environment).

From source repo:

```sh
cargo run -p stbl_cli -- build --preview-open
```

## 8. Where Output Goes

By default, output is generated under cache space:

```text
~/.cache/stbl/<site-name>/out
```

Generated HTML for the front page is typically:

```text
~/.cache/stbl/<site-name>/out/index.html
```

## 9. Typical Authoring Loop

1. Edit Markdown in `articles/`.
2. Run `stbl_cli build` (or preview mode).
3. Refresh browser.
4. Commit both content and configuration changes.

## 10. First-Run Troubleshooting

`stbl_cli: command not found`
- Run through `cargo run -p stbl_cli -- ...` or install the binary.

Build fails with missing media tooling
- Install `ffmpeg` if your content includes video processing.

Front page missing
- Ensure `articles/index.md` exists.

Article does not appear in listings
- Confirm it is published (`published` must not be `no` or `false`).

No visible "updated" timestamp
- `updated` may be disabled (`updated: no` or `updated: false`), or equal/too close to `published` after rounding rules.

## Next Chapter

Continue with:

- `docs/cli.md` for command reference and workflows
- `docs/project-structure.md` for directory/layout conventions
