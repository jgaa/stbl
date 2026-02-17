# Caching

`stbl` uses an optional SQLite task cache to skip unchanged build work.

Media caching is especially important. `stbl` generates scaled and optimized variants for images and videos, and that processing can take significant time on the first run (or whenever media cache entries are invalid). With a warm cache, unchanged media tasks are skipped.

## What Is Cached

The cache stores task-level records:

- `task_id`
- input fingerprint (`[u8; 32]`)
- expected output file paths
- update timestamp (UTC seconds)

At build time, each task can be skipped when:

- cached fingerprint matches current fingerprint, and
- all recorded output files still exist.

## Cache Backend

- Backend: SQLite (`stbl_cache::SqliteCacheStore`)
- Schema versioned internally (`schema_version` in `meta` table)
- On schema mismatch, cache tables are recreated automatically

## Default Cache Location

Default cache root:

```text
~/.cache/stbl/<site.id>/
```

Default paths:

- cache DB: `~/.cache/stbl/<site.id>/cache.sqlite`
- default build output: `~/.cache/stbl/<site.id>/out`

`<site.id>` comes from `stbl.yaml` (`site.id`).

## CLI Controls

Use cache normally:

```sh
stbl_cli build
```

Disable cache for a run:

```sh
stbl_cli build --no-cache
```

Use custom cache DB path:

```sh
stbl_cli build --cache-path /tmp/stbl-cache.sqlite
```

Remove cache/output for current site:

```sh
stbl_cli clean
```

## Build Output Indicators

Build prints cache state and execution stats:

- `cache: on|off`
- `executed: <n>`
- `skipped: <n>`
- `cache_path: <path>` (when applicable)

## Invalidation Behavior

Tasks are re-executed when any relevant input changes, such as:

- source content or metadata affecting task fingerprint
- render-relevant config values
- template hash changes
- media source hash changes
- missing output files

Additional controls:

- `--regenerate-content` forces non-media tasks to run even with cache hits.
- media tasks can still be skipped if cache is valid unless media inputs changed.

## Failure and Fallback Behavior

Cache is best-effort:

- If cache directory cannot be created, build continues with cache off.
- If cache DB cannot be opened, build continues with cache off.
- Warnings are printed, but build does not fail solely due to cache availability.

## Operational Guidance

Use defaults for normal development.

Use `--no-cache` when:

- debugging stale output behavior,
- validating full rebuild behavior,
- benchmarking cold build performance.

Use `stbl_cli clean` when:

- you want to fully reset cached outputs and database for a site.

## Related Chapters

- `docs/cli.md`
- `docs/project-structure.md`
- `docs/troubleshooting.md`
