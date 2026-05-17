# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `examples/cie_csv_to_json` (`csv` feature) — converter that reads the raw
  CIE CSV files from `data/cie-raw/`, prepends rich metadata headers, parses
  with `SpectrumFile::from_csv_str`, and writes JSON.  The 20 converted CIE
  data files (illuminants and colour rendering test samples, CC BY-SA 4.0)
  are published in the
  [spectral-data](https://github.com/harbik/spectral-data) repository under
  `spectra/cie/`.  Includes download instructions for the source files.
- `tests/cie_data.rs` (`csv` feature) — integration tests: schema validation,
  CSV round-trip (JSON → `to_csv` → `from_csv_str`), and spot-checks on
  known values for D65, the CRI 14 samples, and the FL/LED batch files.
  Tests skip gracefully when the data directory is absent.

## [0.3.0] - 2026-05-16

### Added

- `csv` feature (opt-in): `SpectrumFile::from_csv_path` / `from_csv_str` —
  import generic CSV or TSV spectral files with an optional metadata header
  block and one spectrum per data column.
- `csv` feature: `SpectrumFile::to_tsv` / `to_csv` / `write_tsv` / `write_csv`
  — export any `SpectrumFile` to tab- or comma-separated text.
- `SpectrumRecord::resample(target, method)` — resample a spectrum onto a new
  wavelength axis using `ResampleMethod::Linear` (linear interpolation, with
  clamping at the range boundaries), `ResampleMethod::BoxcarAverage`
  (rectangular-window averaging; falls back to linear interpolation for output
  bins that contain no input samples), or `ResampleMethod::Gaussian`
  (Gaussian-kernel weighted average; FWHM taken from
  `metadata.measurement_conditions.spectral_resolution_nm` when present,
  otherwise the mean step size of the target axis). A provenance
  `ProcessingStep` is appended automatically.

### Changed

- `spectrashop` feature is **no longer enabled by default**. Crates that relied
  on the implicit default must now opt in explicitly: add
  `spectral-io = { version = "0.3", features = ["spectrashop"] }` to
  `Cargo.toml`.

## [0.2.0] - 2026-05-14

### Added

- `MeasurementType::Emission` — new variant for gas discharge / emission line spectra.
- `MeasurementType` now derives `Copy` and `Eq`.

## [0.1.0] - 2026-05-12

### Added

- `SpectrumFile` — top-level enum (`Single` / `Batch`) with full JSON schema and
  cross-field validation via `from_path` / `from_str` / `from_str_unchecked`.
- `SpectrumRecord`, `SpectrumMetadata`, `WavelengthAxis`, `SpectralData`,
  `ColorScience`, `Provenance`, `BatchMetadata` — typed representation of the
  `spectrum_file_schema.json` v1.0.0 format.
- `spectrashop` feature (default): `SpectrumFile::from_spectrashop_path` /
  `from_spectrashop_str` — parse the SpectraShop tab-separated text export format.
- `scripts/spectrum_file_schema.json` — JSON Schema for the file format.
- `scripts/spectrum_file_validate.py` — standalone Python validation script.
- `examples/spectrashop_to_json` — CLI converter from SpectraShop `.txt` to
  `spectral-io` JSON, with `-c <copyright>` flag.
- Committed SpectraShop fixture files (`data/spectrashop/`) for five
  representative instrument/material combinations (monitor, thermochromic ink,
  ISCC-NBS centroid charts, candies, ceramics).
- Corresponding JSON conversions in `data/spectral-io/` with Myers copyright
  notice for each committed fixture.
- Unit tests for previously uncovered parser paths: multiple `BEGIN_DATA`
  blocks, `NOTE`/`ACQUIRE_NOTE` provenance, measurement filter preservation,
  `SAMPLE_ID3` and unknown fields in `custom`, European decimal aperture format.

[0.3.0]: https://github.com/harbik/spectral-io/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/harbik/spectral-io/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/harbik/spectral-io/releases/tag/v0.1.0
