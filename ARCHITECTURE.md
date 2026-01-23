# stbl architecture

## Crates

### stbl_core
Pure logic:
- markdown + front matter parsing
- site model (Site, Page, Series, Media)
- rendering pipeline
- NO filesystem writes
- NO sqlite
- NO clap

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
