# CLI Reference

This chapter documents the `stbl_cli` command-line interface.

## Command Shape

```sh
stbl_cli [GLOBAL_OPTIONS] <COMMAND> [COMMAND_OPTIONS] [ARGS]
```

Examples:

```sh
stbl_cli verify
stbl_cli build --preview-open
stbl_cli init --kind blog mysite
```

## Global Options

Available for all commands:

- `-s, --source-dir <SOURCE_DIR>`: Site root directory (defaults to current directory).
- `-v, --verbose`: Verbose output.
- `--unknown-header-keys <error|warn>`: How unknown frontmatter keys are handled (default: `error`).
- `--no-writeback`: Never write header/body write-back changes to source files.
- `--commit-writeback`: Commit write-back changes with git after applying.
- `--include-unpublished`: Include unpublished pages in output, only valid with preview builds.

Behavior constraints:

- `--include-unpublished` requires a preview build (`build --preview` or `build --preview-open`).
- `--commit-writeback` cannot be combined with `--no-writeback`.

## Commands Overview

- `scan`: Parse and assemble content; prints content summary.
- `verify`: Validate config/content without building output.
- `clean`: Remove cached output/database for current site.
- `plan`: Build and print execution plan (optionally Graphviz DOT).
- `build`: Build full output site.
- `upgrade`: Convert legacy `stbl.conf` into `stbl.yaml`.
- `init`: Initialize a new site scaffold.
- `apply-colors`: Apply/derive theme colors in `stbl.yaml`.
- `show-color-themes`: Generate HTML preview of color presets.

## `scan`

Usage:

```sh
stbl_cli scan [ARTICLES_DIR]
```

Options:

- `ARTICLES_DIR`: Content dir, default `articles`.

Typical use:

```sh
stbl_cli scan
```

Notes:

- Non-destructive (no output build).
- Reports page/series counts and write-back summary.

## `verify`

Usage:

```sh
stbl_cli verify [OPTIONS] [ARTICLES_DIR]
```

Options:

- `--strict`: Enable stricter validation checks.
- `ARTICLES_DIR`: Content dir, default `articles`.

Typical use:

```sh
stbl_cli verify --strict
```

Notes:

- Non-destructive.
- Best first check in CI or before publishing.

## `clean`

Usage:

```sh
stbl_cli clean
```

Notes:

- Removes cached outputs/database for this site configuration.
- Safe when you want a fully fresh build next run.

## `plan`

Usage:

```sh
stbl_cli plan [OPTIONS] [ARTICLES_DIR]
```

Options:

- `--dot [PATH]`: Write Graphviz DOT plan file (defaults to `stbl.dot` if flag is provided without path).
- `ARTICLES_DIR`: Content dir, default `articles`.

Examples:

```sh
stbl_cli plan
stbl_cli plan --dot
stbl_cli plan --dot build-plan.dot
```

Notes:

- Non-destructive.
- Useful for inspecting task graph and dependencies.

## `build`

Usage:

```sh
stbl_cli build [OPTIONS] [ARTICLES_DIR]
```

Core options:

- `--out <PATH>`: Override output directory.
- `--publish-to <DEST>`: Publish after successful build.
- `--no-cache`: Disable cache for this run.
- `--cache-path <PATH>`: Override cache DB/path.
- `--regenerate-content`: Force regeneration of content outputs.
- `--jobs <N>`: Parallel workers (`N >= 1`).

Media/compression:

- `--fast-images`: Faster image mode.
- `--precompress <true|false>`: Enable gzip/brotli precompressed files (default `true`).
- `--fast-compress`: Faster/lower-level compression.

Preview:

- `--preview`: Run local preview server.
- `--preview-open`: Run preview and open browser.
- `--preview-host <HOST>`: Default `127.0.0.1`.
- `--preview-port <PORT>`: Default `8080`.
- `--preview-index <FILE>`: Default `index.html`.

Notification:

- `--beep`: Beep after build.
- `--no-beep`: Disable beep.

Examples:

```sh
stbl_cli build
stbl_cli build --preview-open
stbl_cli build --out ./dist --no-cache
stbl_cli build --publish-to example.com:/var/www/site
```

Behavior notes:

- `--preview-open` implies preview mode.
- In preview mode, write-back runs in dry-run mode (`would modify ...`) rather than writing files.
- `--publish-to` requires `publish.command` in `stbl.yaml`.

Publish command token replacement:

- `{{local-site}}` -> resolved output directory path
- `{{destination}}` -> value passed to `--publish-to`

## `upgrade`

Usage:

```sh
stbl_cli upgrade [--force]
```

Notes:

- Reads legacy `stbl.conf`.
- Writes `stbl.yaml`.
- Use `--force` to overwrite when needed.

## `init`

Usage:

```sh
stbl_cli init [OPTIONS] [TARGET_DIR]
```

Options:

- `--title <TITLE>`: Default `Demo Site`.
- `--url <URL>`: Default `http://localhost:8080/`.
- `--language <LANGUAGE>`: Default `en`.
- `--kind <blog|landing-page>`: Default `blog`.
- `--color-theme <NAME>`: Apply preset during init.
- `--copy-all`: Copy full template/content set.

Examples:

```sh
stbl_cli init
stbl_cli init mysite
stbl_cli init --kind landing-page --title "Product Site" mysite
```

## `apply-colors`

Usage:

```sh
stbl_cli apply-colors [OPTIONS] [NAME]
```

Preset workflow:

- `--list-presets`: List available preset names.
- `NAME`: Preset to apply to `stbl.yaml`.

Derived workflow (`--from-base`):

- Requires `--bg` and `--accent`.
- Optional tuning: `--fg`, `--link`, `--heading`, `--mode`, `--brand`.

Safety options:

- `--dry-run`: Print resulting YAML instead of writing.
- `--backup`: Write `stbl.yaml.bak` before update.

Examples:

```sh
stbl_cli apply-colors --list-presets
stbl_cli apply-colors nord
stbl_cli apply-colors --from-base --bg "#ffffff" --accent "#0055cc" --dry-run
```

## `show-color-themes`

Usage:

```sh
stbl_cli show-color-themes [OPTIONS]
```

Options:

- `--out <PATH>`: Output HTML path.
- `--open`: Start local preview and open result.

Examples:

```sh
stbl_cli show-color-themes
stbl_cli show-color-themes --out /tmp/themes.html --open
```

## Recommended Workflows

Fast local loop:

1. `stbl_cli verify`
2. `stbl_cli build --preview-open`

Pre-release check:

1. `stbl_cli verify --strict`
2. `stbl_cli plan --dot`
3. `stbl_cli build`

Publish:

1. Ensure `publish.command` is configured in `stbl.yaml`.
2. `stbl_cli build --publish-to <destination>`

## Exit Behavior

- Validation and build errors exit non-zero.
- Some commands print warnings but still succeed if no fatal error occurred.
- `verify` may return non-zero depending on findings, especially under `--strict`.
