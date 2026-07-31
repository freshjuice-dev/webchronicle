# Contributing

Thanks for considering a contribution. This is a small project — the process is lightweight.

## How to contribute

1. **Check existing issues** on [GitHub](https://github.com/freshjuice-dev/webchronicle/issues) first. If your idea or bug isn't listed, open one.
2. **Fork the repo** and **create a branch** from `main`: `git checkout -b fix-sitemap-detection` — keep branch names short and descriptive.
3. **Make your change.** Touch only what the task needs. No drive-by refactors.
4. **Verify locally:**
   ```sh
   cargo build
   cargo test
   ```
   Both must pass. If `build` fails, fix it before pushing.
5. **Open a pull request** to `main`. Include the CLA acceptance line in the PR description.
6. **Wait for review.** A maintainer will look at it.

## What we accept

- Bug fixes — scraper, server, rewrite engine, templates.
- New sitemap discovery patterns or asset download improvements.
- Accessibility and high-contrast mode improvements.
- Documentation improvements.

## What we don't accept

- Features that require external dependencies without justification — keep the binary lean.
- Telemetry, analytics, or cloud sync — this is a local-first tool.
- Changes to the license or licensing model.

## Code style

- Rust. Follow the patterns already in the file you're editing.
- One-liner comments for *why*, not *what*.
- Keep diffs small. One PR per concern.

## CLA

External contributions require the [CLA](./CLA.md).

Copy this line into the PR description:

```text
I have read and agree to the CLA.
```

No signature bot — that line in the PR body is the acceptance. Maintainers will not merge without it.

## Project facts

- License of this repo: AGPL-3.0-or-later
- Contact: contact@freshjuice.dev
- Primary forge: GitHub (`github.com/freshjuice-dev/webchronicle`)