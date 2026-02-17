# stbl 2.0.0

stbl 2.0.0 is a full rewrite of the now-obsolete C++ implementation, rebuilt entirely in Rust.

The motivation for switching to Rust was not a belief that everything must be rewritten in Rust. Rather, it was the realization that the C++ version required major refactoring—similar in scope to a full rewrite. Since Rust applications are generally easier to build (no endless iterations with Jenkins or GitHub Actions just to make minimal-dependency builds work) and distribute (typically a single static binary—no DLL hell), I decided to give it a try.

I used OpenAI’s *Codex* agent for most of the implementation. This was not a recreational coding project, but a simple, well-defined application that I urgently needed to have ready “yesterday.”

## Highlights

* Complete Rust rewrite of the stbl engine and CLI
* Modernized architecture with a clear separation between core rendering, cache, and CLI
* Continued focus on fast static site builds, media processing, and template-driven output

## Rust Version

* Rust edition: 2024
* Minimum Supported Rust Version (MSRV): 1.85

