# stbl2 Media Handling Specification (Images & Video)

This document formalizes **media handling semantics** in stbl2. It preserves legacy stbl behavior while fitting the new architecture (pure core + planned execution + deterministic output).

This spec is **normative** for Milestone 3 (Assets + media pipeline) and later milestones.

---

## 1. Goals

Media handling in stbl2 must:

* Preserve legacy authoring workflows
* Produce deterministic, cacheable outputs
* Support responsive images and scaled video
* Work without JavaScript (progressive enhancement)
* Allow templates to fully control presentation

---

## 2. Source Media Layout (Authoring Model)

Source media lives in **well-known top-level directories**:

* `images/` — raster and vector images
* `video/` — video sources

Authors reference media using:

* Header fields (e.g. `banner:`)
* Extended Markdown image syntax

Derived media **must not** be written into the source tree by default.

---

## 3. Header Field: `banner`

### Semantics

* `banner` expects an **image name**, not a path
* The value may include an extension, but does not have to

Example:

```
banner: amazed-banner
```

### Resolution rules

During build execution (CLI layer):

1. If an extension is provided, resolve exactly
2. Otherwise probe `images/` for known extensions in order:

   * `.avif`, `.webp`, `.jpg`, `.png`

If no match is found, this is an error.

### Rendering

* The resolved image is treated as a managed image
* Scaled variants are generated
* The template renders the banner using a `<picture>` element

---

## 4. Extended Markdown Image / Video Syntax

stbl2 extends standard Markdown image syntax:

```
![alt text](path;attr1;attr2;...)
```

### Parsing rules

* The destination is split on `;`
* The first segment is the media path
* Remaining segments are attributes

### Media type detection

* `images/...` → managed image
* `video/...` → managed video
* Anything else → passed through unchanged

---

## 5. Image Attributes

Supported attributes:

| Attribute | Meaning                              |
| --------- | ------------------------------------ |
| `banner`  | Full-width presentation role         |
| `NN%`     | Preferred display width (e.g. `70%`) |

Attributes affect **layout and sizing**, not which variants are generated.

Example:

```
![Screenshot](images/example.png;70%)
```

---

## 6. Video Attributes

Supported attributes:

| Attribute | Meaning                       |
| --------- | ----------------------------- |
| `p360`    | Prefer 360p variant           |
| `p480`    | Prefer 480p variant           |
| `p720`    | Prefer 720p variant (default) |
| `p1080`   | Prefer 1080p variant          |
| `p1440`   | Prefer 1440p variant          |
| `p2160`   | Prefer 2160p variant          |

Example:

```
![Intro video](video/intro.mp4;p360)
```

---

## 7. Image Processing

### Variant generation

Images are processed into a **variant set**.

Default widths (configurable):

```
[94, 128, 248, 360, 480, 640, 720, 950, 1280, 1440, 1680, 1920, 2560]
```

Variants are generated for configured formats (e.g. AVIF/WebP/JPEG).
Default format set is **AVIF + WebP + JPEG/PNG fallback**.
Fast mode (`--fast-images`) skips AVIF and emits **WebP + JPEG/PNG fallback**.

### Output

* Originals are copied to `out/images/`
* Variants are written to `out/images/_scale_<width>/`
* Filenames include width and content hash for generated variants

### HTML output

Images are rendered as `<picture>` with:

* `<source>` elements per format
* `srcset` with width descriptors
* `sizes` computed from site breakpoints and max body width

The browser selects the best variant.

---

## 8. Video Processing

### Variant generation

Videos may be:

* Copied through unchanged (minimum)
* Transcoded into configured height variants

Default heights:

```
[360, 480, 720, 1080]
```

A poster image is always generated.

### Output

* Video originals and variants: `out/video/` and `out/video/_scale_<height>/`
* Posters: `out/video/_poster_/`

---

## 9. Video HTML Output (Progressive Enhancement)

stbl2 **always emits valid HTML5 video**.

Example:

```html
<figure class="video" data-stbl-video data-prefer="p360">
  <video controls preload="metadata" poster="poster.jpg">
    <source src="video-360.mp4" type="video/mp4">
    <source src="video-720.mp4" type="video/mp4">
    Your browser doesn’t support HTML5 video —
    <a href="video-720.mp4">download it</a>.
  </video>
</figure>
```

Rules:

* Preferred resolution is listed first
* Higher resolutions may follow
* Works fully with JavaScript disabled

---

## 10. JavaScript Enhancement (Template Responsibility)

Templates may:

* Add JS to enhance video UI
* Wrap or decorate existing `<video>` elements

Templates **must not** remove or replace the base HTML5 `<video>`.

Core output must remain playable without JS.

---

## 11. Determinism and Rebuild Rules

* Media outputs are identified by:

  * Source content hash
  * Variant parameters (size, format, quality)
* Missing or stale variants are regenerated
* Source trees are never modified by default

Legacy `_scale_*` workflows may be supported via optional compatibility flags.

---

## 12. Non-goals (for now)

* Adaptive streaming (HLS/DASH)
* Automatic discovery of media
* Writing derived media into source directories by default

---

**End of specification**
