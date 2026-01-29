# Validation routine

Run the quick validation script from the repo root:

```
./scripts/validate.sh
```

What it does:
- Runs `cargo fmt --all` and `cargo test --all`.
- Builds the pagination fixture into `/tmp/stbl-out`.
- Checks for:
  - `index.html` existence
  - at least one page-2-or-higher output
  - no empty meta spans
  - YYYY-MM-DD date format in the blog index
  - at least one tag page under `tags/`

Output:
- `/tmp/stbl-out`
