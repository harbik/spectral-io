# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- Example files: `black_ace_licorice` (simple, single-spectrum),
  `wallace_china` (average, 22-spectrum batch),
  `apple_13_inch_monitor` (complex, sub-nm irradiance, 4-channel).

[0.1.0]: https://github.com/harbik/spectral-io/releases/tag/v0.1.0
