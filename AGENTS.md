# Repository Guidelines

## Project Structure & Module Organization
- `crates/stbl_core`: Pure rendering and parsing logic; no filesystem writes, no SQLite, no CLI wiring.
- `crates/stbl_cache`: Optional incremental cache layer (SQLite + BLAKE3).
- `crates/stbl_cli`: CLI entry point; config loading, filesystem walking, wiring core + cache.
- `doc/`: Architecture notes and diagrams.
- `examples/`: Sample site content, templates, and assets.
- `target/`: Cargo build artifacts (generated).

## Build, Test, and Development Commands
- `cargo build`: Build the workspace.
- `cargo build -p stbl_cli`: Build only the CLI crate.
- `cargo run -p stbl_cli -- --help`: Run the CLI and show usage.
- `cargo test`: Run all tests in the workspace.
- `cargo test -p stbl_core`: Run core unit + integration tests.

## Coding Style & Naming Conventions
- Rust 2024 edition workspace; default `rustfmt` style (no repo-specific config found).
- Keep functions small and testable; avoid global state.
- Respect module boundaries: core logic stays in `stbl_core`; filesystem and CLI stay in `stbl_cli`.
- Parsing code must include tests.

## Testing Guidelines
- Uses Rust’s built-in test harness (`#[test]`).
- Unit tests live alongside code; integration tests live under `crates/stbl_core/tests/`.
- Name tests descriptively by behavior (e.g., `parse_header_*`, `url_*`).

## Commit & Pull Request Guidelines
- Commit messages are short, sentence‑case statements (e.g., “Add URL style abstraction…”).
- PRs should include: purpose/summary, tests run (or “not run” with reason), and any behavior changes.
- Include example inputs/outputs or screenshots for changes that affect rendered HTML.

## Configuration & Security Notes
- Site configuration and content layout are described in the README (e.g., `stbl.conf`, `articles/`, `templates/`).
- The cache is optional and should be safe to remove without affecting correctness.

## Agent-Specific Instructions
- Follow `.codex.md` rules: no new crates without request, no filesystem access in `stbl_core`, no TODOs, no feature creep.

## Execution & IO boundaries (important)

- `stbl_core` MUST remain pure:
  - No filesystem access
  - No network access
  - No environment inspection
- All filesystem writes MUST happen in `stbl_cli`.

## Planning vs execution

- `scan` and `plan` are ALWAYS non-destructive.
- Header write-back may only occur during `build`.
- Build planning must fully describe outputs before execution.
- Executors must rely only on planned output artifacts.

## URL handling

- URL semantics are defined by `stbl_core::url::UrlMapper`.
- No code outside `url.rs` may construct `.html` paths or `index.html` paths manually.
- All links and output paths must go through the mapper.
