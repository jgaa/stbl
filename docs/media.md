# Media

This chapter covers image/video handling in `stbl`: source conventions, Markdown syntax, processing, and output.

## Supported Source Roots

`stbl` recognizes media paths in Markdown when they start with:

- `images/` for images
- `video/` for videos

Paths outside these prefixes are not treated as managed media variants.

## Markdown Media Syntax

Use standard Markdown image syntax:

```markdown
![Alt text](images/photo.jpg)
![Demo clip](video/intro.mp4)
```

You can append `;` options in the destination.

Image option examples:

```markdown
![Hero](images/hero.jpg;banner)
![Figure](images/chart.png;maxw=40rem;maxh=60vh)
![Inline](images/logo.png;80%)
```

Video option examples:

```markdown
![Launch](video/launch.mov;p720;maxw=70vw)
```

Recognized options:

- Images:
  - `banner`
  - `maxw=<length>`
  - `maxh=<length>`
  - `<N>%` where `N` is 1..100
- Videos:
  - `p360|p480|p720|p1080|p1440|p2160` (preferred quality hint)
  - `maxw=<length>`
  - `maxh=<length>`

Allowed length units for `maxw/maxh`:

- `px`, `rem`, `em`, `%`, `vw`, `vh`

Invalid `maxw/maxh` values are reported as parsing errors.

## Image Processing

For discovered image sources, build planning creates:

- copy original output (`images/...`)
- scaled variants under `images/_scale_<width>/...`

Widths come from:

- `media.images.widths` in `stbl.yaml`

Format behavior:

- normal mode: WebP + fallback (`jpg` or `png`), optionally AVIF if built with AVIF feature
- fast mode (`--fast-images`): skips AVIF path and uses faster image mode
- alpha images use PNG fallback; non-alpha use JPEG fallback

SVG behavior:

- SVG images are copied as-is (no raster resize variants).

## Video Processing

For discovered video sources, build planning creates:

- copy original video (`video/...`)
- poster image at `video/_poster_/...jpg`
- scaled MP4 variants at `video/_scale_<height>/...mp4`
- MP4 compatibility output for non-MP4 inputs (`video/...mp4`)

Heights come from:

- `media.video.heights` in `stbl.yaml`

Poster extraction time:

- `media.video.poster_time_sec` in config

Video metadata probing and transcoding use `ffmpeg`.

## Required Tooling

- Images: handled internally by Rust image stack.
- Videos: require `ffmpeg` available on `PATH` for probe/transcode/poster extraction.

If `ffmpeg` is missing and videos are used, media steps fail.

## Banner Images

Header field:

```yaml
banner: hero
```

Resolution rules:

- `banner: images/file.ext` works if file exists.
- bare name with extension resolves from `images/<name>`.
- bare name without extension tries: `avif`, `webp`, `jpg`, `png`.
- banner values containing path separators (except `images/...`) are rejected.

Theme wide background images:

- `theme.wide_background.image` may reference a local relative file (commonly `images/...`).
- HTTP/data/url(...) values are ignored for local file discovery.
- local file path must exist or build fails.

## Config Keys

Relevant sections in `stbl.yaml`:

- `media.images.widths`
- `media.images.quality`
- `media.video.heights`
- `media.video.poster_time`
- `banner.widths`
- `banner.quality`
- `banner.align`
- `theme.wide_background.image`

## Troubleshooting

`image not found ...`:
- verify source file exists under `images/`.

`video not found ...`:
- verify source file exists under `video/`.

`failed to run ffmpeg ...`:
- install `ffmpeg` and ensure it is on `PATH`.

Unexpected missing variant:
- check configured width/height limits and source dimensions (variants larger than source are skipped).

## Related Chapters

- `docs/project-structure.md`
- `docs/templates-and-themes.md`
- `docs/troubleshooting.md`
