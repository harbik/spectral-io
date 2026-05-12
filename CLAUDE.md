# Claude Code instructions for spectral-io

## Relationship with colorimetry

`spectral-io` was born inside the
[colorimetry](https://github.com/harbik/colorimetry) workspace as a crate
called `colorimetry-io`. It was split out in May 2026 to become a standalone
library with no dependency on `colorimetry`, so that any application dealing
with optical spectral data can use it without carrying a full CIE colorimetry
implementation.

The two repositories share a common author (Gerard Harbers / Harbers Bik LLC)
and are developed together at `~/Projects/colorimetry` and
`~/Projects/spectral-io` on the same machine. They are kept as separate repos
and separate crates on crates.io.

**Dependency direction:** `spectral-io` has no knowledge of `colorimetry`.
`colorimetry` has an _optional_ dependency on `spectral-io` (feature
`"spectral-io"`), which it uses to expose the `IntoSpectrum` extension trait
that converts a `spectral_io::SpectrumRecord` into a
`colorimetry::Spectrum`. This trait lives in
`colorimetry/src/spectrum/from_spectral_io.rs`.

When making changes that affect the public types in `spectral-io`
(field additions, renames, new enum variants), check whether the
`from_spectral_io.rs` module in the colorimetry repo needs updating too.
The `[patch.crates-io]` entry in `colorimetry/Cargo.toml` points to
`../spectral-io` for local development, so both can be tested together
without publishing.

## Pre-release / CI checks

Whenever code is modified or tests are run, always execute these commands in
order and report any errors before marking work complete:

```sh
cargo fmt --check               # formatting
cargo clippy --all-targets --all-features -- -D warnings   # lints
cargo check --all-targets       # build check
cargo test --all-features       # tests with all features
cargo test --no-default-features  # tests without optional features
cargo doc --no-deps --all-features 2>&1 | grep -i warning  # doc warnings
```

Feature-gated code must compile both with and without the feature.
`cargo test` is run for both `--all-features` and `--no-default-features`
because the `spectrashop` feature is on by default and must not break
the no-feature build.

## Coding conventions

- Prefer editing existing files over creating new ones.
- Do not add docstrings, comments, or type annotations to code that was not
  changed.
- Private helper functions must not be linked from public doc comments —
  rustdoc rejects them with `--deny warnings`. Use plain backtick notation
  instead: `` `ss_parse` ``.
- Feature-gated code must compile both with and without the feature.

## Release process

### 1. Update CHANGELOG.md

Move every entry under `## [Unreleased]` into a new dated section:

```markdown
## [0.2.0] - 2026-06-01
```

Add a diff link at the bottom following the existing pattern.

### 2. Bump version number

Change `version` in `Cargo.toml`. There is only one crate, so one file to
change.

Also update `colorimetry/Cargo.toml` (in the colorimetry repo) if the
`spectral-io` version pin there needs bumping.

### 3. Run checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo doc --no-deps --all-features
```

All must pass with zero errors and zero warnings.

### 4. Commit and tag

```sh
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: release v0.2.0"
git tag v0.2.0
git push origin main --tags
```

Create a GitHub Release from the tag and paste the relevant CHANGELOG
section as the release notes.

### 5. Publish to crates.io

```sh
cargo publish
```

After publishing, update the version pin in `colorimetry/Cargo.toml` and
verify the colorimetry build still passes.

### Per-PR changelog notes

- `CHANGELOG.md` should be updated for any user-facing change, not just at
  release time.
- The crate is pre-1.0; breaking changes are allowed but must be noted in the
  changelog.
- Deprecated items use `#[deprecated(since = "x.y.z", note = "use X instead")]`.

## Tests

- Tests that depend on optional features must be gated with
  `#[cfg(feature = "...")]`.
- Integration tests in `tests/spectrashop_data.rs` require the Chromaxion
  spectral library to be present at `data/spectrashop/` (gitignored). All
  tests in that file guard against the missing directory with an early
  `return` so they silently skip in CI rather than panic.
- Do not mock internals. Tests hit real computation paths.

## SpectraShop data

The directory `data/spectrashop/` is gitignored. It holds spectral data files
from the [Chromaxion Spectral Library](https://www.chromaxion.com/spectral-library.php)
by Robin Myers Imaging, which are subject to the following terms:

- **Personal, scientific, and teaching use** is free.
- **Redistribution** requires attribution to *Chromaxion.com* or *Robin Myers*.
- **Commercial sale** requires express written permission from Robin Myers.

All data © Robin D. Myers, all rights reserved worldwide.
Contact <robin@rmimaging.com> for commercial licensing enquiries.

The three example files committed to `examples/` (Black Ace Licorice, Wallace
China, Apple 13-inch Monitor) are redistributed with attribution as permitted
by the above terms.
