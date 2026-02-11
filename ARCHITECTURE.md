# stbl architecture

## Crates

### stbl_core

Pure logic crate.

Responsibilities:
- Markdown parsing + header parsing
- Site model normalization (`Project`, `Page`, `Series`)
- URL semantics (`UrlMapper`)
- Build planning (`BuildPlan`, DAG of tasks + outputs)
- Pure rendering helpers (Markdown → HTML, templates)

Non-responsibilities:
- No filesystem IO
- No SQLite
- No CLI parsing
- No execution of build steps


### stbl_cache
Incremental build cache:
- SQLite only
- Uses BLAKE3 hashes
- Cache is optional and must be safely removable
- No rendering logic

### stbl_cli
User interface:
- clap
- config loading
- filesystem walking
- wiring core + cache

### Build lifecycle

1. scan (stbl_cli)
   - Walk filesystem
   - Parse documents
   - No writes

2. assemble (stbl_core)
   - Normalize metadata
   - Resolve series, tags, authors
   - Assign missing parts/published timestamps (record intent only)

3. plan (stbl_core)
   - Produce BuildPlan (tasks + outputs + dependencies)
   - Deterministic, backend-agnostic

4. execute (stbl_cli)
   - Create directories
   - Render HTML, RSS, sitemap
   - Perform header write-back (if allowed)

## Series

A series is a group of articles under a directory with its own `index.md`. For example:

```
articles/my-series/index.md
articles/my-series/part-1.md
articles/my-series/part-2.md
```

Behavior and rules:
- The series cover page is `index.md` in the series directory.
- All other `.md` files in that directory are series parts.
- Part numbers come from the header `part` field; if missing, they are assigned in assemble and written back when allowed.
- Series index pages are rendered to `series-dir/index.html` even when `site.url_style` is `html`.
- Series parts keep their normal output paths based on source structure.
