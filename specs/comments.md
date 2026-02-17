# Comments (Optional)

`stbl` supports optional, per-article comments with a simple provider
configuration in `stbl.yaml` and a per-article header switch.

## Enable per article

Add `comments` in the article header:

```
---
title: Example
comments: disqus
---
```

To disable comments for a specific page:

```
comments: no
```

If the header is omitted, `stbl` falls back to `comments.default` from
the site configuration.

## Configure providers

Define providers under the `comments` section in `stbl.yaml`:

```
comments:
  default: disqus
  disqus:
    template: templates/disqus.html
    src: https://example.disqus.com/embed.js
```

### Template resolution

`comments.<provider>.template` can be:

- An inline template string (contains `<`, `{{`, or newlines), or
- A file path relative to the site root.

If a relative path does not resolve, `stbl` also checks `templates/<path>`
for compatibility with legacy setups.

### Template variables

Templates use `{{...}}` placeholders. The following are always provided:

- `{{uuid}}` (if present in the header)
- `{{title}}`
- `{{page-url}}` (absolute URL)
- `{{page_url}}` (same as `page-url`)

Provider-specific variables are exposed as `{{provider-key}}` for each
key defined in the provider block. Example:

```
comments:
  disqus:
    template: templates/disqus.html
    src: https://example.disqus.com/embed.js
```

In `templates/disqus.html`:

```
<script src="{{disqus-src}}"></script>
```

## Notes

- If the provider or template is missing, comments are skipped for that page.
- The default page template renders comments after the content and series
  navigation.
