# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[0.2.0]: https://github.com/harbik/spectral-io/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/harbik/spectral-io/releases/tag/v0.1.0
