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
