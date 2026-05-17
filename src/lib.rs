//! `spectral-io` reads, writes, and validates optical spectral data files.
//! It defines a compact JSON format — `spectrum_file_schema.json` v1.0.0 — for
//! UV-Vis and visible-range measurements, designed to be suitable for color-science
//! calculations, long-term archiving, and data exchange between instruments,
//! pipelines, and applications.
//!
//! The format captures everything a downstream calculation needs in one place:
//! the measured spectrum, the physical conditions under which it was taken
//! (instrument, geometry, illuminant, observer), and an optional provenance trail.
//! The crate also ships support for additional formats:
//!
//! - **CSV / TSV** (`csv` feature) — generic delimited text with an optional
//!   `KEY: VALUE` metadata header block; import via [`SpectrumFile::from_csv_path`]
//!   / [`SpectrumFile::from_csv_str`], export via [`SpectrumFile::to_tsv`] /
//!   [`SpectrumFile::to_csv`].
//! - **SpectraShop** (`spectrashop` feature) — the tab-separated
//!   text format used to distribute the
//!   [Chromaxion Spectral Library](https://www.chromaxion.com/spectral-library.php),
//!   one of the largest freely available collections of measured spectra; import
//!   via [`SpectrumFile::from_spectrashop_path`] /
//!   [`SpectrumFile::from_spectrashop_str`].
//!
//! ## Quick start
//!
//! ### Reading a JSON file
//!
//! ```no_run
//! use spectral_io::SpectrumFile;
//!
//! let file = SpectrumFile::from_path("spectrum.json").expect("could not load file");
//! for sp in file.spectra() {
//!     let (min_nm, max_nm) = sp.wavelength_range_nm().unwrap();
//!     println!("{}: {} points, {:.0}–{:.0} nm", sp.id, sp.n_points(), min_nm, max_nm);
//! }
//! ```
//!
//! ### Importing from other formats
//!
//! - CSV / TSV (`csv` feature):
//!
//! ```no_run
//! # #[cfg(feature = "csv")]
//! # {
//! use spectral_io::SpectrumFile;
//!
//! let file = SpectrumFile::from_csv_path("measurements.tsv")
//!     .expect("could not parse file");
//! let tsv = file.to_tsv();
//! # }
//! ```
//!
//! - SpectraShop (`spectrashop` feature):
//!
//! ```no_run
//! # #[cfg(feature = "spectrashop")]
//! # {
//! use spectral_io::SpectrumFile;
//!
//! let file = SpectrumFile::from_spectrashop_path("Munsell Matte 1994.txt")
//!     .expect("could not parse SpectraShop file");
//! println!("{} spectra imported", file.spectra().len());
//! # }
//! ```
//!
//! ### Resampling to an equidistant grid
//!
//! ```no_run
//! use spectral_io::{SpectrumFile, ResampleMethod, WavelengthAxis, WavelengthRange};
//!
//! let file = SpectrumFile::from_path("spectrum.json").unwrap();
//! let target = WavelengthAxis {
//!     range_nm: Some(WavelengthRange { start: 380.0, end: 780.0, interval: 10.0 }),
//!     values_nm: None,
//! };
//! for sp in file.spectra() {
//!     let resampled = sp.resample(&target, ResampleMethod::Linear);
//!     println!("{}: {} points", resampled.id, resampled.n_points());
//! }
//! ```
//!
//! ### Serialising back to JSON
//!
//! Any [`SpectrumFile`] can be round-tripped through `serde_json`:
//!
//! ```no_run
//! # use spectral_io::SpectrumFile;
//! # let file = SpectrumFile::from_path("spectrum.json").unwrap();
//! let json = serde_json::to_string_pretty(&file).expect("serialisation failed");
//! std::fs::write("output.json", json).unwrap();
//! ```
//!
//! ## Cargo features
//!
//! | Feature | Default | Description |
//! |---|---|---|
//! | `spectrashop` | no | Enables [`SpectrumFile::from_spectrashop_path`] and [`SpectrumFile::from_spectrashop_str`] |
//! | `csv` | no | Enables [`SpectrumFile::from_csv_path`], [`SpectrumFile::from_csv_str`], [`SpectrumFile::to_tsv`], [`SpectrumFile::to_csv`], [`SpectrumFile::write_tsv`], and [`SpectrumFile::write_csv`] |
//!
//! ## Error handling
//!
//! All fallible entry points return `Result<_, `[`SpectrumFileError`]`>`.
//! [`SpectrumFileError`] has four variants:
//!
//! - **`Io`** — file not found or unreadable.
//! - **`Json`** — not valid JSON.
//! - **`SchemaValidation`** — structural problems: missing required fields, wrong
//!   types, unknown enum values. All errors for the whole file are collected and
//!   returned together so you see every problem at once, not just the first.
//! - **`CrossFieldValidation`** — inter-field constraint failures: wavelength/value
//!   array length mismatches, non-monotonic wavelengths, reflectance values outside
//!   `[0, 1]`, a `"custom"` illuminant without its spectral power distribution, etc.
//!   Again all errors are collected before returning.
//!
//! Use [`SpectrumFile::from_str_unchecked`] to bypass both validation passes when
//! you are certain the source is well-formed (e.g. data you just wrote yourself).
//!
//! ## File format
//!
//! Files are JSON objects with a `schema_version` (semver string) and a
//! `file_type` of either `"single"` or `"batch"`.
//!
//! ### Single file
//!
//! Use a single file when the measurement session produced exactly one spectrum —
//! for example a single colour patch or a one-off transmission measurement.
//! The spectrum lives directly under the key `"spectrum"`:
//!
//! ```json
//! {
//!   "schema_version": "1.0.0",
//!   "file_type": "single",
//!   "spectrum": { "id": "patch-01", "metadata": { "..." }, "..." }
//! }
//! ```
//!
//! ### Batch file
//!
//! Use a batch file when multiple spectra share common conditions — a colour
//! chart, a paint swatch book, or a series of time-series measurements. An
//! optional `"batch_metadata"` block carries fields common to the whole set
//! (title, operator, instrument, measurement conditions) so they do not need
//! to be repeated on every spectrum:
//!
//! ```json
//! {
//!   "schema_version": "1.0.0",
//!   "file_type": "batch",
//!   "batch_metadata": { "title": "Munsell Matte 1994", "date": "1994-01-01" },
//!   "spectra": [ { "id": "5R 4/2", "..." }, { "id": "5YR 4/2", "..." } ]
//! }
//! ```
//!
//! ### SpectrumRecord object
//!
//! Each spectrum has four top-level sections (two required, two optional).
//!
//! #### `metadata` (required)
//!
//! Descriptive information about what was measured and how. `measurement_type`
//! and `date` are the only required sub-fields; everything else is optional but
//! strongly encouraged for reproducibility.
//!
//! | Field | Type | Notes |
//! |---|---|---|
//! | `measurement_type` | string enum | `reflectance`, `transmittance`, `absorbance`, `radiance`, `irradiance`, `emission`, `sensitivity` |
//! | `date` | string | ISO 8601 date (`YYYY-MM-DD`) |
//! | `title` | string | optional human-readable name for the sample |
//! | `sample_id` | string | optional machine-readable sample identifier |
//! | `operator` | string | optional name or ID of the person who measured |
//! | `instrument` | object | optional: `manufacturer`, `model`, `serial_number`, `detector_type`, `light_source` |
//! | `measurement_conditions` | object | optional: `integration_time_ms`, `averaging`, `temperature_celsius`, `geometry`, `specular_component`, `spectral_resolution_nm`, `measurement_aperture_mm`, `measurement_filter` |
//! | `surface` | string | optional surface finish of the specimen (e.g. `"Matte"`, `"Gloss"`, `"Semigloss"`) |
//! | `sample_backing` | string | optional backing used behind the specimen during measurement (e.g. `"Black"`, `"White"`) |
//! | `tags` | string[] | optional free-form labels for search and filtering |
//! | `copyright` | string | optional copyright notice (e.g. `"© 2024 Acme Lab"`) |
//! | `custom` | object | optional user-defined key/value pairs for application-specific metadata |
//!
//! #### `wavelength_axis` (required)
//!
//! Exactly one of `values_nm` or `range_nm` must be present — not both, not
//! neither. Use `range_nm` for the common case of an evenly-spaced grid (e.g.
//! 380–780 nm at 10 nm steps); it is more compact and unambiguous. Use
//! `values_nm` when the instrument produces an irregular grid or when the
//! spacing is not constant.
//!
//! | Field | Type | Notes |
//! |---|---|---|
//! | `values_nm` | number[] | explicit wavelength list in nm; min 2 entries, strictly increasing |
//! | `range_nm` | object | evenly-spaced grid: `start`, `end`, and `interval` (all in nm) |
//!
//! #### `spectral_data` (required)
//!
//! The measured values, one per wavelength point. For reflectance and
//! transmittance, values must lie in `[0, 1]` when `scale` is `"fractional"`
//! (the default), or in `[0, 100]` when `scale` is `"percent"`. There is no
//! range constraint for `absorbance`, `radiance`, or `irradiance`.
//!
//! | Field | Type | Notes |
//! |---|---|---|
//! | `values` | number[] | measured values, one per wavelength point |
//! | `uncertainty` | number[] | optional per-point 1-σ uncertainty, same length as `values` |
//! | `scale` | string enum | `"fractional"` (0–1, default) or `"percent"` (0–100) |
//!
//! #### `color_science` (optional)
//!
//! Metadata needed to perform CIE colorimetric calculations from the spectral
//! data — the illuminant under which the sample is viewed, the observer (colour
//! matching functions), and an optional white reference. Pre-computed colorimetric
//! results (`XYZ`, `Lab`, CCT, …) may also be stored here as a convenience cache;
//! the spectral data is always the authoritative source.
//!
//! | Field | Type | Notes |
//! |---|---|---|
//! | `illuminant` | string enum | `D65`, `D50`, `D55`, `D75`, `A`, `B`, `C`, `F1`–`F12`, `LED-*`, or `"custom"` |
//! | `illuminant_custom_sd` | object | required when `illuminant` is `"custom"`; provide the SPD as `wavelengths_nm` and `values` arrays |
//! | `cie_observer` | string enum | `"CIE 1931 2 degree"` (default), `"CIE 1964 10 degree"`, `"CIE 2015 2 degree"`, `"CIE 2015 10 degree"` |
//! | `white_reference` | object | optional calibration tile description and spectral reflectance values |
//! | `results` | object | optional pre-computed colorimetric values — informational only |
//!
//! Available `results` sub-fields (all optional):
//!
//! | Field | Type | Notes |
//! |---|---|---|
//! | `XYZ` | `[number, number, number]` | CIE tristimulus values [X, Y, Z] |
//! | `xy` | `[number, number]` | CIE 1931 chromaticity coordinates [x, y] |
//! | `uv_prime` | `[number, number]` | CIE 1976 UCS chromaticity [u′, v′] |
//! | `Lab` | `[number, number, number]` | CIELAB [L\*, a\*, b\*] |
//! | `CCT_K` | number | Correlated colour temperature in Kelvin |
//! | `Duv` | number | Distance from the Planckian locus (signed, CIE 1960 UCS) |
//!
//! #### `provenance` (optional)
//!
//! An audit trail recording where the data came from and what has been done to
//! it. Particularly valuable when spectra have been converted from another
//! format, averaged, smoothed, or trimmed. Fields: `software`,
//! `software_version`, `source_file`, `source_format`, `notes`, and an ordered
//! `processing_steps` array (each step has a `step` name, `description`, and
//! optional `parameters` object).
//!
//! ### Full single-spectrum example
//!
//! A reflectance measurement of Munsell chip 5R 4/2, measured with a
//! Konica Minolta CM-700d in diffuse/8° geometry, stored as a regular
//! 380–780 nm grid at 10 nm intervals:
//!
//! ```json
//! {
//!   "schema_version": "1.0.0",
//!   "file_type": "single",
//!   "spectrum": {
//!     "id": "chip-5R-4-2",
//!     "metadata": {
//!       "measurement_type": "reflectance",
//!       "date": "2026-04-01",
//!       "title": "Munsell 5R 4/2",
//!       "instrument": { "manufacturer": "Konica Minolta", "model": "CM-700d" },
//!       "measurement_conditions": { "geometry": "d:8", "specular_component": "excluded" }
//!     },
//!     "wavelength_axis": {
//!       "range_nm": { "start": 380, "end": 780, "interval": 10 }
//!     },
//!     "spectral_data": {
//!       "values": [0.048, 0.051, 0.054, 0.058, 0.063],
//!       "scale": "fractional"
//!     },
//!     "color_science": {
//!       "illuminant": "D65",
//!       "cie_observer": "CIE 1931 2 degree",
//!       "results": {
//!         "XYZ": [17.35, 9.12, 1.18],
//!         "xy": [0.629, 0.330],
//!         "Lab": [36.1, 55.7, 37.2]
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! ## Validation
//!
//! [`SpectrumFile::from_path`] and [`SpectrumFile::from_json_str`] run two validation
//! passes before returning:
//!
//! 1. **Schema validation** — checks required fields, correct types, and that
//!    enum fields (`measurement_type`, `illuminant`, `cie_observer`,
//!    `scale`) contain only allowed values.
//! 2. **Cross-field validation** — checks that `values_nm` and `values` have
//!    equal length; that `uncertainty` (if present) has the same length; that
//!    wavelengths are strictly increasing; that reflectance/transmittance values
//!    lie in `[0, 1]` when `scale` is `"fractional"`; and that a custom
//!    illuminant is accompanied by `illuminant_custom_sd`.
//!
//! Both passes collect all errors before returning, so a single call surfaces
//! every problem in the file at once. Use [`SpectrumFile::from_str_unchecked`]
//! to skip validation entirely when the source is fully trusted.
//!
//! ## Importing SpectraShop files
//!
//! [SpectraShop](https://www.chromaxion.com/) is measurement and colour-analysis
//! software by Robin Myers Imaging. Its tab-separated text export format (`.txt`)
//! is used to distribute the
//! [Chromaxion Spectral Library](https://www.chromaxion.com/spectral-library.php),
//! which contains measured reflectance, transmittance, and irradiance spectra for
//! hundreds of real-world materials — paint colours, Munsell chips, colour charts,
//! photographic filters, monitor primaries, fabrics, inks, and more.
//!
//! Requires the `spectrashop` feature.
//! [`SpectrumFile::from_spectrashop_path`] and [`SpectrumFile::from_spectrashop_str`]
//! parse the format and convert each data record in the `BEGIN_DATA`/`END_DATA`
//! block into a [`SpectrumRecord`]. File-level metadata (spectrum type, illuminant,
//! observer, geometry, etc.) is applied to every record. A file with one record
//! returns [`SpectrumFile::Single`]; two or more return [`SpectrumFile::Batch`].
//!
//! The `spectrashop_to_json` example binary converts a SpectraShop file to
//! the `spectral-io` JSON format and can optionally embed a copyright notice:
//!
//! ```text
//! cargo run --example spectrashop_to_json -- -c "© Author" input.txt output.json
//! ```
//!
//! ### Format and data licensing
//!
//! The SpectraShop text format is proprietary to Robin Myers Imaging. A format
//! specification is published at
//! <https://www.chromaxion.com/spectral_library/SpectraShop_Import-Export_Format.pdf>
//! specifically to permit third-party readers and writers.
//!
//! Spectral data files from the
//! [Chromaxion Spectral Library](https://www.chromaxion.com/spectral-library.php)
//! are subject to the following terms:
//!
//! - **Personal, scientific, and teaching use** is free.
//! - **Redistribution** requires attribution to *Chromaxion.com* or *Robin Myers*.
//! - **Commercial sale** of the data in any form requires express written
//!   permission from Robin Myers.
//!
//! All data © Robin D. Myers, all rights reserved worldwide.
//! Contact <robin@rmimaging.com> for commercial licensing enquiries.
//!
//! ## Importing and exporting CSV / TSV files
//!
//! Requires the `csv` feature.
//!
//! [`SpectrumFile::from_csv_path`] and [`SpectrumFile::from_csv_str`] read a
//! generic delimited text file. The delimiter (tab or comma) is auto-detected.
//! Files have two sections:
//!
//! 1. **Header block** — zero or more `KEY: VALUE` metadata lines (or
//!    `KEY = VALUE`; or `KEY<delim>VALUE` for a set of recognised keywords).
//!    Lines starting with `#` and blank lines are ignored throughout.
//!
//! 2. **Data block** — the first row whose first cell parses as a number
//!    (wavelength in nm) starts the data block. The immediately preceding
//!    non-blank line (if non-numeric) is the optional column-header row.
//!    First column = wavelength; each further column becomes one
//!    [`SpectrumRecord`].
//!
//! A file with one data column returns [`SpectrumFile::Single`]; two or more
//! return [`SpectrumFile::Batch`].
//!
//! ```text
//! Measurement_Type: reflectance
//! Date: 2026-05-15
//! Illuminant: D65
//!
//! wavelength_nm    patch_A    patch_B
//! 380    0.041    0.089
//! 390    0.052    0.092
//! 400    0.063    0.095
//! ```
//!
//! [`SpectrumFile::to_tsv`] and [`SpectrumFile::to_csv`] serialise back to
//! tab- or comma-separated text, writing `KEY: VALUE` metadata lines so files
//! round-trip cleanly. [`SpectrumFile::write_tsv`] and
//! [`SpectrumFile::write_csv`] write directly to a file path.

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[cfg(feature = "spectrashop")]
mod spectrashop;

#[cfg(feature = "csv")]
mod csv_text;

mod resample;
pub use resample::ResampleMethod;

// ─────────────────────────────────────────────────────────────────────────────
// Error type
// ─────────────────────────────────────────────────────────────────────────────

/// All errors that can occur while loading or validating a spectrum file.
#[derive(Debug, Error)]
pub enum SpectrumFileError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// Structural schema violation (wrong type, missing required field,
    /// value not in allowed enum set, etc.)
    #[error("Schema validation failed:\n{0}")]
    SchemaValidation(String),

    /// Cross-field constraint violation (array length mismatch,
    /// non-monotonic wavelengths, value out of physical range, etc.)
    #[error("Cross-field validation failed:\n{0}")]
    CrossFieldValidation(String),
}

pub type Result<T> = std::result::Result<T, SpectrumFileError>;

// ─────────────────────────────────────────────────────────────────────────────
// Top-level file enum
// ─────────────────────────────────────────────────────────────────────────────

/// The top-level structure of a spectrum JSON file.
/// Tagged by `file_type`: either `"single"` or `"batch"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "file_type", rename_all = "snake_case")]
pub enum SpectrumFile {
    Single {
        schema_version: String,
        spectrum: Box<SpectrumRecord>,
    },
    Batch {
        schema_version: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_metadata: Option<Box<BatchMetadata>>,
        spectra: Vec<SpectrumRecord>,
    },
}

impl SpectrumFile {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Load and fully validate a UV-Vis JSON file from a file path.
    /// Runs structural schema validation then cross-field checks.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        Self::from_json_str(&raw)
    }

    /// Load and fully validate a UV-Vis JSON file from a JSON string.
    pub fn from_json_str(json: &str) -> Result<Self> {
        // 1. Parse into untyped Value for structural checks
        let value: serde_json::Value = serde_json::from_str(json)?;

        // 2. Structural / schema-level validation
        validate_schema(&value)?;

        // 3. Deserialise into typed structs
        let file: SpectrumFile = serde_json::from_value(value)?;

        // 4. Cross-field validation
        file.validate_cross_fields()?;

        Ok(file)
    }

    /// Deserialise without any validation. Useful when you fully trust the source.
    pub fn from_str_unchecked(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// Returns all spectra in the file (works for both single and batch).
    pub fn spectra(&self) -> Vec<&SpectrumRecord> {
        match self {
            SpectrumFile::Single { spectrum, .. } => vec![spectrum.as_ref()],
            SpectrumFile::Batch { spectra, .. } => spectra.iter().collect(),
        }
    }

    /// The schema version declared in the file.
    pub fn schema_version(&self) -> &str {
        match self {
            SpectrumFile::Single { schema_version, .. } => schema_version,
            SpectrumFile::Batch { schema_version, .. } => schema_version,
        }
    }

    /// Batch metadata, if this is a batch file.
    pub fn batch_metadata(&self) -> Option<&BatchMetadata> {
        match self {
            SpectrumFile::Batch { batch_metadata, .. } => batch_metadata.as_deref(),
            _ => None,
        }
    }

    // ── Cross-field validation ────────────────────────────────────────────────

    fn validate_cross_fields(&self) -> Result<()> {
        let mut errors: Vec<String> = Vec::new();

        for sp in self.spectra() {
            let id = &sp.id;
            let wl = sp.wavelength_axis.wavelengths_nm();
            let vals = &sp.spectral_data.values;

            // wavelength count == value count
            if wl.len() != vals.len() {
                errors.push(format!(
                    "SpectrumRecord '{id}': wavelength_axis has {} points \
                     but spectral_data.values has {} — must match.",
                    wl.len(),
                    vals.len()
                ));
            }

            // uncertainty length == value count
            if let Some(u) = &sp.spectral_data.uncertainty {
                if u.len() != vals.len() {
                    errors.push(format!(
                        "SpectrumRecord '{id}': spectral_data.uncertainty has {} points \
                         but spectral_data.values has {} — must match.",
                        u.len(),
                        vals.len()
                    ));
                }
            }

            // wavelengths strictly increasing
            if wl.windows(2).any(|w| w[0] >= w[1]) {
                errors.push(format!(
                    "SpectrumRecord '{id}': wavelength_axis is not strictly increasing."
                ));
            }

            // reflectance / transmittance in [0,1] when scale = fractional
            let scale = sp.spectral_data.scale.as_deref().unwrap_or("fractional");
            let is_bounded = matches!(
                sp.metadata.measurement_type,
                MeasurementType::Reflectance | MeasurementType::Transmittance
            );
            if is_bounded && scale == "fractional" {
                let bad: Vec<f64> = vals
                    .iter()
                    .copied()
                    .filter(|&v| !(0.0..=1.0).contains(&v))
                    .collect();
                if !bad.is_empty() {
                    errors.push(format!(
                        "SpectrumRecord '{id}': measurement_type={:?}, scale='fractional' \
                         but {} value(s) fall outside [0,1]. First offender: {}",
                        sp.metadata.measurement_type,
                        bad.len(),
                        bad[0]
                    ));
                }
            }

            // custom illuminant requires illuminant_custom_sd
            if let Some(cs) = &sp.color_science {
                if cs.illuminant.as_deref() == Some("custom") && cs.illuminant_custom_sd.is_none() {
                    errors.push(format!(
                        "SpectrumRecord '{id}': color_science.illuminant is 'custom' \
                         but illuminant_custom_sd is missing."
                    ));
                }
                if let Some(csd) = &cs.illuminant_custom_sd {
                    if csd.wavelengths_nm.len() != csd.values.len() {
                        errors.push(format!(
                            "SpectrumRecord '{id}': illuminant_custom_sd.wavelengths_nm ({}) \
                             and .values ({}) must have equal length.",
                            csd.wavelengths_nm.len(),
                            csd.values.len()
                        ));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(SpectrumFileError::CrossFieldValidation(errors.join("\n")))
        }
    }
}

impl std::str::FromStr for SpectrumFile {
    type Err = SpectrumFileError;
    fn from_str(s: &str) -> Result<Self> {
        Self::from_json_str(s)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Structs — mirror the JSON schema
// ─────────────────────────────────────────────────────────────────────────────

/// A single spectral measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumRecord {
    pub id: String,
    pub metadata: SpectrumMetadata,
    pub wavelength_axis: WavelengthAxis,
    pub spectral_data: SpectralData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_science: Option<ColorScience>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

impl SpectrumRecord {
    /// Returns all `(wavelength_nm, value)` pairs.
    pub fn points(&self) -> Vec<(f64, f64)> {
        self.wavelength_axis
            .wavelengths_nm()
            .into_iter()
            .zip(self.spectral_data.values.iter().copied())
            .collect()
    }

    /// Wavelength range as `(min_nm, max_nm)`, or `None` if the axis is empty.
    pub fn wavelength_range_nm(&self) -> Option<(f64, f64)> {
        let wl = self.wavelength_axis.wavelengths_nm();
        Some((*wl.first()?, *wl.last()?))
    }

    /// Number of spectral data points.
    pub fn n_points(&self) -> usize {
        self.spectral_data.values.len()
    }
}

/// Descriptive metadata for one spectrum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumMetadata {
    pub measurement_type: MeasurementType,
    /// ISO 8601 date (YYYY-MM-DD).
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument: Option<Instrument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_conditions: Option<MeasurementConditions>,
    /// Type of surface for a reflective specimen (e.g. `"Matte"`, `"Gloss"`, `"Semigloss"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    /// Backing used behind the sample during measurement (e.g. `"Black"`, `"White"`, `"Substrate"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_backing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Copyright notice for this spectrum (e.g. `"© 2024 Acme Lab"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<serde_json::Value>,
}

/// The physical quantity measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementType {
    Reflectance,
    Transmittance,
    Absorbance,
    Radiance,
    Irradiance,
    Emission,
    /// Dimensionless spectral sensitivity or response function — colour matching
    /// functions, cone fundamentals, luminous efficiency V(λ), action spectra.
    /// Values are not constrained to [0, 1].
    Sensitivity,
}

/// Minimal instrument identification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instrument {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detector_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_source: Option<String>,
}

/// Physical conditions under which the measurement was made.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementConditions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integration_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub averaging: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_celsius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specular_component: Option<SpecularComponent>,
    /// Optical (spectral) resolution of the instrument in nm, typically the FWHM of the slit function.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectral_resolution_nm: Option<f64>,
    /// Instrument measurement aperture size in mm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_aperture_mm: Option<f64>,
    /// Filter used on the spectrometer during measurement (e.g. `"UV Block"`, `"Polarizer"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_filter: Option<String>,
}

/// Whether the specular component is included or excluded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecularComponent {
    Included,
    Excluded,
    #[serde(rename = "not applicable")]
    NotApplicable,
}

/// The wavelength axis of the measurement. All values are in nm.
///
/// Exactly one of `values_nm` or `range_nm` must be present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavelengthAxis {
    /// Explicit wavelength list in nm. Use for irregular grids.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values_nm: Option<Vec<f64>>,
    /// Evenly-spaced grid descriptor. Use for regular grids.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_nm: Option<WavelengthRange>,
}

impl WavelengthAxis {
    /// Returns the wavelength values in nm, expanding `range_nm` if that variant is used.
    pub fn wavelengths_nm(&self) -> Vec<f64> {
        if let Some(v) = &self.values_nm {
            v.clone()
        } else if let Some(r) = &self.range_nm {
            r.expand()
        } else {
            vec![]
        }
    }
}

/// Evenly-spaced wavelength grid defined by start, end, and interval (all in nm).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavelengthRange {
    pub start: f64,
    pub end: f64,
    pub interval: f64,
}

impl WavelengthRange {
    /// Expands the range into an explicit list of wavelength values in nm.
    pub fn expand(&self) -> Vec<f64> {
        // Use floor with a small epsilon so floating-point imprecision never
        // produces an extra step beyond `end` (e.g. 40.9999… flooring to 40,
        // not 41 after rounding).
        let n = ((self.end - self.start) / self.interval + 1e-9).floor() as usize + 1;
        (0..n)
            .map(|i| self.start + i as f64 * self.interval)
            .collect()
    }
}

/// The measured spectral values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralData {
    pub values: Vec<f64>,
    /// Optional per-point uncertainty (1 standard deviation), same length as `values`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncertainty: Option<Vec<f64>>,
    /// `"fractional"` (0–1) or `"percent"` (0–100). Defaults to `"fractional"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<String>,
}

/// Metadata required for CIE colorimetry and color-science calculations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScience {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub illuminant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub illuminant_custom_sd: Option<CustomIlluminantSd>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cie_observer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white_reference: Option<WhiteReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<ColorScienceResults>,
}

/// Pre-computed colorimetric results derived from the spectral data.
///
/// All fields are optional and informational — the spectral data is always the authoritative
/// source. Any subset may be present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScienceResults {
    /// CIE tristimulus values [X, Y, Z].
    #[serde(rename = "XYZ", skip_serializing_if = "Option::is_none")]
    pub xyz: Option<[f64; 3]>,
    /// CIE 1931 chromaticity coordinates [x, y].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xy: Option<[f64; 2]>,
    /// CIE 1976 UCS chromaticity coordinates [u′, v′].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uv_prime: Option<[f64; 2]>,
    /// CIELAB coordinates [L*, a*, b*].
    #[serde(rename = "Lab", skip_serializing_if = "Option::is_none")]
    pub lab: Option<[f64; 3]>,
    /// Correlated color temperature in Kelvin.
    #[serde(rename = "CCT_K", skip_serializing_if = "Option::is_none")]
    pub cct_k: Option<f64>,
    /// Distance from the Planckian locus (signed) in the CIE 1960 UCS.
    #[serde(rename = "Duv", skip_serializing_if = "Option::is_none")]
    pub duv: Option<f64>,
}

/// Spectral power distribution for a custom illuminant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomIlluminantSd {
    pub wavelengths_nm: Vec<f64>,
    pub values: Vec<f64>,
}

/// White reference / calibration tile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhiteReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calibration_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_values: Option<Vec<f64>>,
}

/// Processing history and software trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_steps: Option<Vec<ProcessingStep>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// A single processing step applied to the raw data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingStep {
    pub step: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Optional metadata common to all spectra in a batch file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument: Option<Instrument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_conditions: Option<MeasurementConditions>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Structural schema validator (pure serde_json, no external crate)
// ─────────────────────────────────────────────────────────────────────────────
//
// Checks that are enforced here (equivalent to JSON Schema):
//   - Required top-level fields present and correct type
//   - file_type is "single" or "batch"
//   - schema_version matches semver pattern
//   - Each spectrum has required fields (id, metadata, wavelength_axis, spectral_data)
//   - measurement_type is one of the allowed enum values
//   - scale, if present, is "fractional" or "percent"
//   - values_nm has at least 2 entries
//   - All numeric arrays contain only numbers

const ALLOWED_MEASUREMENT_TYPES: &[&str] = &[
    "reflectance",
    "transmittance",
    "absorbance",
    "radiance",
    "irradiance",
    "emission",
    "sensitivity",
];

const ALLOWED_ILLUMINANTS: &[&str] = &[
    "D65", "D50", "D55", "D75", "A", "B", "C", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8",
    "F9", "F10", "F11", "F12", "LED-B1", "LED-B2", "LED-B3", "LED-B4", "LED-B5", "LED-BH1",
    "LED-RGB1", "LED-V1", "LED-V2", "custom",
];

const ALLOWED_OBSERVERS: &[&str] = &[
    "CIE 1931 2 degree",
    "CIE 1964 10 degree",
    "CIE 2015 2 degree",
    "CIE 2015 10 degree",
];

fn validate_schema(v: &serde_json::Value) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    // Top-level must be an object
    let obj = match v.as_object() {
        Some(o) => o,
        None => {
            return Err(SpectrumFileError::SchemaValidation(
                "Root value must be a JSON object.".into(),
            ))
        }
    };

    // schema_version: required, string, semver-ish
    match obj.get("schema_version") {
        None => errors.push("Missing required field: schema_version".into()),
        Some(sv) => {
            if !sv.is_string() {
                errors.push("schema_version must be a string".into());
            } else {
                let s = sv.as_str().unwrap();
                let parts: Vec<&str> = s.split('.').collect();
                if parts.len() != 3 || parts.iter().any(|p| p.parse::<u32>().is_err()) {
                    errors.push(format!(
                        "schema_version '{s}' does not look like semver (e.g. 1.0.0)"
                    ));
                }
            }
        }
    }

    // file_type: required, "single" or "batch"
    let file_type = match obj.get("file_type") {
        None => {
            errors.push("Missing required field: file_type".into());
            None
        }
        Some(ft) => match ft.as_str() {
            Some(s @ "single") | Some(s @ "batch") => Some(s.to_string()),
            Some(other) => {
                errors.push(format!(
                    "file_type must be 'single' or 'batch', got '{other}'"
                ));
                None
            }
            None => {
                errors.push("file_type must be a string".into());
                None
            }
        },
    };

    match file_type.as_deref() {
        Some("single") => {
            match obj.get("spectrum") {
                None => errors.push("Single file must have a 'spectrum' field".into()),
                Some(sp) => validate_spectrum(sp, "spectrum", &mut errors),
            }
            if obj.contains_key("spectra") {
                errors.push(
                    "Single file must not have a 'spectra' array (use file_type='batch')".into(),
                );
            }
        }
        Some("batch") => {
            match obj.get("spectra") {
                None => errors.push("Batch file must have a 'spectra' array".into()),
                Some(arr) => match arr.as_array() {
                    None => errors.push("'spectra' must be an array".into()),
                    Some(items) => {
                        if items.is_empty() {
                            errors.push("'spectra' array must not be empty".into());
                        }
                        for (i, sp) in items.iter().enumerate() {
                            validate_spectrum(sp, &format!("spectra[{i}]"), &mut errors);
                        }
                    }
                },
            }
            if obj.contains_key("spectrum") {
                errors.push("Batch file must not have a 'spectrum' field".into());
            }
        }
        _ => {} // already reported
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(SpectrumFileError::SchemaValidation(errors.join("\n")))
    }
}

fn validate_spectrum(v: &serde_json::Value, path: &str, errors: &mut Vec<String>) {
    let obj = match v.as_object() {
        Some(o) => o,
        None => {
            errors.push(format!("{path}: must be an object"));
            return;
        }
    };

    // id: required string
    require_string(obj, "id", path, errors);

    // metadata: required object
    if let Some(meta) = require_object(obj, "metadata", path, errors) {
        validate_metadata(meta, &format!("{path}.metadata"), errors);
    }

    // wavelength_axis: required object
    if let Some(wa) = require_object(obj, "wavelength_axis", path, errors) {
        validate_wavelength_axis(wa, &format!("{path}.wavelength_axis"), errors);
    }

    // spectral_data: required object
    if let Some(sd) = require_object(obj, "spectral_data", path, errors) {
        validate_spectral_data(sd, &format!("{path}.spectral_data"), errors);
    }

    // color_science: optional
    if let Some(cs) = obj.get("color_science") {
        if let Some(cso) = cs.as_object() {
            validate_color_science(cso, &format!("{path}.color_science"), errors);
        } else {
            errors.push(format!("{path}.color_science must be an object"));
        }
    }
}

fn validate_metadata(
    obj: &serde_json::Map<String, serde_json::Value>,
    path: &str,
    errors: &mut Vec<String>,
) {
    // measurement_type: required, enum
    match obj.get("measurement_type") {
        None => errors.push(format!("{path}: missing required field 'measurement_type'")),
        Some(mt) => match mt.as_str() {
            None => errors.push(format!("{path}.measurement_type must be a string")),
            Some(s) if !ALLOWED_MEASUREMENT_TYPES.contains(&s) => errors.push(format!(
                "{path}.measurement_type '{s}' is not allowed. Must be one of: {}",
                ALLOWED_MEASUREMENT_TYPES.join(", ")
            )),
            _ => {}
        },
    }
    // date: required string
    require_string(obj, "date", path, errors);
}

fn validate_wavelength_axis(
    obj: &serde_json::Map<String, serde_json::Value>,
    path: &str,
    errors: &mut Vec<String>,
) {
    let has_values = obj.contains_key("values_nm");
    let has_range = obj.contains_key("range_nm");

    match (has_values, has_range) {
        (false, false) => {
            errors.push(format!(
                "{path}: exactly one of 'values_nm' or 'range_nm' must be present (neither found)"
            ));
            return;
        }
        (true, true) => {
            errors.push(format!(
                "{path}: exactly one of 'values_nm' or 'range_nm' must be present (both found)"
            ));
            return;
        }
        _ => {}
    }

    if has_values {
        match obj.get("values_nm").and_then(|v| v.as_array()) {
            None => errors.push(format!("{path}.values_nm must be an array")),
            Some(items) => {
                if items.len() < 2 {
                    errors.push(format!("{path}.values_nm must have at least 2 elements"));
                }
                if items.iter().any(|x| !x.is_number()) {
                    errors.push(format!("{path}.values_nm must contain only numbers"));
                }
            }
        }
    } else {
        match obj.get("range_nm").and_then(|v| v.as_object()) {
            None => errors.push(format!("{path}.range_nm must be an object")),
            Some(r) => {
                for field in ["start", "end", "interval"] {
                    match r.get(field) {
                        None => errors
                            .push(format!("{path}.range_nm: missing required field '{field}'")),
                        Some(v) if !v.is_number() => {
                            errors.push(format!("{path}.range_nm.{field} must be a number"))
                        }
                        _ => {}
                    }
                }
                if let Some(iv) = r.get("interval").and_then(|v| v.as_f64()) {
                    if iv <= 0.0 {
                        errors.push(format!("{path}.range_nm.interval must be positive"));
                    }
                }
            }
        }
    }
}

fn validate_spectral_data(
    obj: &serde_json::Map<String, serde_json::Value>,
    path: &str,
    errors: &mut Vec<String>,
) {
    // values: required, array of numbers, min 2
    match obj.get("values") {
        None => errors.push(format!("{path}: missing required field 'values'")),
        Some(arr) => match arr.as_array() {
            None => errors.push(format!("{path}.values must be an array")),
            Some(items) => {
                if items.len() < 2 {
                    errors.push(format!("{path}.values must have at least 2 elements"));
                }
                if items.iter().any(|x| !x.is_number()) {
                    errors.push(format!("{path}.values must contain only numbers"));
                }
            }
        },
    }

    // uncertainty: optional array of non-negative numbers
    if let Some(unc) = obj.get("uncertainty") {
        match unc.as_array() {
            None => errors.push(format!("{path}.uncertainty must be an array")),
            Some(items) => {
                if items.iter().any(|x| !x.is_number()) {
                    errors.push(format!("{path}.uncertainty must contain only numbers"));
                } else if items.iter().any(|x| x.as_f64().unwrap_or(0.0) < 0.0) {
                    errors.push(format!("{path}.uncertainty values must be non-negative"));
                }
            }
        }
    }

    // scale: optional, enum
    if let Some(sc) = obj.get("scale") {
        match sc.as_str() {
            None => errors.push(format!("{path}.scale must be a string")),
            Some(s) if s != "fractional" && s != "percent" => errors.push(format!(
                "{path}.scale must be 'fractional' or 'percent', got '{s}'"
            )),
            _ => {}
        }
    }
}

fn validate_color_science(
    obj: &serde_json::Map<String, serde_json::Value>,
    path: &str,
    errors: &mut Vec<String>,
) {
    // illuminant: optional, enum
    if let Some(il) = obj.get("illuminant") {
        match il.as_str() {
            None => errors.push(format!("{path}.illuminant must be a string")),
            Some(s) if !ALLOWED_ILLUMINANTS.contains(&s) => errors.push(format!(
                "{path}.illuminant '{s}' is not a recognised CIE illuminant"
            )),
            _ => {}
        }
    }

    // cie_observer: optional, enum
    if let Some(obs) = obj.get("cie_observer") {
        match obs.as_str() {
            None => errors.push(format!("{path}.cie_observer must be a string")),
            Some(s) if !ALLOWED_OBSERVERS.contains(&s) => errors.push(format!(
                "{path}.cie_observer '{s}' not recognised. Must be one of: {}",
                ALLOWED_OBSERVERS.join(", ")
            )),
            _ => {}
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn require_string(
    obj: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    path: &str,
    errors: &mut Vec<String>,
) {
    match obj.get(key) {
        None => errors.push(format!("{path}: missing required field '{key}'")),
        Some(v) if !v.is_string() => errors.push(format!("{path}.{key} must be a string")),
        _ => {}
    }
}

fn require_object<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    path: &str,
    errors: &mut Vec<String>,
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    match obj.get(key) {
        None => {
            errors.push(format!("{path}: missing required field '{key}'"));
            None
        }
        Some(v) => match v.as_object() {
            None => {
                errors.push(format!("{path}.{key} must be an object"));
                None
            }
            Some(o) => Some(o),
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SpectraShop text-format importer
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "spectrashop")]
impl SpectrumFile {
    /// Load a SpectraShop text-export file from a path.
    ///
    /// Parses the SpectraShop tab-separated text format (`.txt`) and converts each
    /// data record into a [`SpectrumRecord`]. File-level metadata (illuminant, observer,
    /// geometry, etc.) is applied to every record. Returns [`SpectrumFile::Single`]
    /// for one record or [`SpectrumFile::Batch`] for multiple records.
    ///
    /// Non-UTF-8 bytes (e.g. Latin-1 encoded files) are replaced with U+FFFD.
    pub fn from_spectrashop_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)?;
        let raw = String::from_utf8_lossy(&bytes).into_owned();
        let filename = path.file_name().and_then(|f| f.to_str());
        spectrashop::ss_parse(&raw, filename)
    }

    /// Parse a SpectraShop text-export string.
    ///
    /// See [`SpectrumFile::from_spectrashop_path`] for format details.
    pub fn from_spectrashop_str(input: &str) -> Result<Self> {
        spectrashop::ss_parse(input, None)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CSV / TSV importer and exporter
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "csv")]
impl SpectrumFile {
    /// Load a CSV or TSV spectral data file from a path.
    ///
    /// The delimiter (tab or comma) is auto-detected. An optional header block
    /// of `KEY: VALUE` lines precedes the data. The first row whose first cell
    /// parses as a number starts the data block; the immediately preceding
    /// non-blank line (if non-numeric) is treated as the column-header row.
    /// First data column = wavelength in nm; each subsequent column becomes one
    /// [`SpectrumRecord`]. Returns [`SpectrumFile::Single`] for one data column
    /// or [`SpectrumFile::Batch`] for multiple.
    pub fn from_csv_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path)?;
        let filename = path.file_name().and_then(|f| f.to_str());
        csv_text::csv_parse(&raw, filename)
    }

    /// Parse a CSV or TSV spectral data string.
    ///
    /// See [`SpectrumFile::from_csv_path`] for format details.
    pub fn from_csv_str(input: &str) -> Result<Self> {
        csv_text::csv_parse(input, None)
    }

    /// Serialise to a tab-separated string.
    ///
    /// Writes a `KEY: VALUE` metadata header derived from the first spectrum,
    /// followed by a column-header row and one data row per wavelength point.
    /// For a batch file all spectra are written as parallel columns sharing the
    /// wavelength axis of the first spectrum.
    pub fn to_tsv(&self) -> String {
        csv_text::csv_write(self, '\t')
    }

    /// Serialise to a comma-separated string.
    ///
    /// See [`SpectrumFile::to_tsv`] for format details.
    pub fn to_csv(&self) -> String {
        csv_text::csv_write(self, ',')
    }

    /// Write a tab-separated file to the given path.
    pub fn write_tsv<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        Ok(std::fs::write(path, self.to_tsv())?)
    }

    /// Write a comma-separated file to the given path.
    pub fn write_csv<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        Ok(std::fs::write(path, self.to_csv())?)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_single(mtype: &str, wls: &[f64], vals: &[f64]) -> String {
        let wl_s: Vec<String> = wls.iter().map(|w| w.to_string()).collect();
        let v_s: Vec<String> = vals.iter().map(|v| v.to_string()).collect();
        format!(
            r#"{{"schema_version":"1.0.0","file_type":"single","spectrum":{{"id":"t1",
            "metadata":{{"measurement_type":"{mtype}","date":"2026-04-29"}},
            "wavelength_axis":{{"values_nm":[{wl}]}},
            "spectral_data":{{"values":[{v}]}}}}}}"#,
            mtype = mtype,
            wl = wl_s.join(","),
            v = v_s.join(","),
        )
    }

    fn wls_41() -> Vec<f64> {
        (0..41).map(|i| 380.0 + i as f64 * 10.0).collect()
    }
    fn vals_41() -> Vec<f64> {
        (0..41).map(|i| i as f64 / 100.0).collect()
    }

    #[test]
    fn valid_single_spectrum() {
        let file = SpectrumFile::from_json_str(&make_single("reflectance", &wls_41(), &vals_41()))
            .unwrap();
        let spectra = file.spectra();
        assert_eq!(spectra.len(), 1);
        assert_eq!(spectra[0].n_points(), 41);
        assert_eq!(file.schema_version(), "1.0.0");
    }

    #[test]
    fn valid_batch_file() {
        let json = r#"{"schema_version":"1.0.0","file_type":"batch","spectra":[
            {"id":"a","metadata":{"measurement_type":"reflectance","date":"2026-04-29"},
             "wavelength_axis":{"values_nm":[380,390,400]},
             "spectral_data":{"values":[0.1,0.2,0.3]}},
            {"id":"b","metadata":{"measurement_type":"transmittance","date":"2026-04-29"},
             "wavelength_axis":{"values_nm":[380,390,400]},
             "spectral_data":{"values":[0.5,0.6,0.7]}}
        ]}"#;
        let file = SpectrumFile::from_json_str(json).unwrap();
        assert_eq!(file.spectra().len(), 2);
    }

    #[test]
    fn missing_measurement_type_is_schema_error() {
        let json = r#"{"schema_version":"1.0.0","file_type":"single","spectrum":{"id":"x",
            "metadata":{"date":"2026-04-29"},
            "wavelength_axis":{"values_nm":[380,390,400]},
            "spectral_data":{"values":[0.1,0.2,0.3]}}}"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::SchemaValidation(_))
        ));
    }

    #[test]
    fn invalid_measurement_type_is_schema_error() {
        let json = make_single("fluorescence", &[380.0, 390.0], &[0.1, 0.2]);
        assert!(matches!(
            SpectrumFile::from_json_str(&json),
            Err(SpectrumFileError::SchemaValidation(_))
        ));
    }

    #[test]
    fn wavelength_value_length_mismatch() {
        let wls = vec![380.0, 390.0, 400.0];
        let vals = vec![0.1, 0.2]; // too short
        assert!(matches!(
            SpectrumFile::from_json_str(&make_single("reflectance", &wls, &vals)),
            Err(SpectrumFileError::CrossFieldValidation(_))
        ));
    }

    #[test]
    fn non_monotonic_wavelengths() {
        let wls = vec![380.0, 370.0, 400.0];
        let vals = vec![0.1, 0.2, 0.3];
        assert!(matches!(
            SpectrumFile::from_json_str(&make_single("reflectance", &wls, &vals)),
            Err(SpectrumFileError::CrossFieldValidation(_))
        ));
    }

    #[test]
    fn reflectance_out_of_range() {
        let wls = vec![380.0, 390.0, 400.0];
        let vals = vec![0.1, 1.5, 0.3];
        assert!(matches!(
            SpectrumFile::from_json_str(&make_single("reflectance", &wls, &vals)),
            Err(SpectrumFileError::CrossFieldValidation(_))
        ));
    }

    #[test]
    fn absorbance_above_one_is_ok() {
        // Absorbance is not bounded by [0,1]
        let wls = vec![380.0, 390.0, 400.0];
        let vals = vec![0.1, 1.8, 2.5];
        assert!(SpectrumFile::from_json_str(&make_single("absorbance", &wls, &vals)).is_ok());
    }

    #[test]
    fn custom_illuminant_missing_sd() {
        let json = r#"{"schema_version":"1.0.0","file_type":"single","spectrum":{"id":"x",
            "metadata":{"measurement_type":"reflectance","date":"2026-04-29"},
            "wavelength_axis":{"values_nm":[380,390,400]},
            "spectral_data":{"values":[0.1,0.2,0.3]},
            "color_science":{"illuminant":"custom"}}}"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::CrossFieldValidation(_))
        ));
    }

    #[test]
    fn points_iterator_correct() {
        let wls = vec![380.0, 390.0, 400.0];
        let vals = vec![0.1, 0.2, 0.3];
        let file = SpectrumFile::from_json_str(&make_single("reflectance", &wls, &vals)).unwrap();
        let pts = file.spectra()[0].points();
        assert_eq!(pts, vec![(380.0, 0.1), (390.0, 0.2), (400.0, 0.3)]);
    }

    #[test]
    fn wavelength_range_accessor() {
        let file = SpectrumFile::from_json_str(&make_single("reflectance", &wls_41(), &vals_41()))
            .unwrap();
        assert_eq!(
            file.spectra()[0].wavelength_range_nm(),
            Some((380.0, 780.0))
        );
    }

    #[test]
    fn invalid_scale_value() {
        let json = r#"{"schema_version":"1.0.0","file_type":"single","spectrum":{"id":"x",
            "metadata":{"measurement_type":"reflectance","date":"2026-04-29"},
            "wavelength_axis":{"values_nm":[380,390,400]},
            "spectral_data":{"values":[0.1,0.2,0.3],"scale":"ratio"}}}"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::SchemaValidation(_))
        ));
    }

    // ── WavelengthAxis and WavelengthRange unit tests ─────────────────────────

    #[test]
    fn wavelength_axis_values_nm_variant() {
        let axis = WavelengthAxis {
            values_nm: Some(vec![380.0, 450.0, 550.0, 700.0]),
            range_nm: None,
        };
        assert_eq!(axis.wavelengths_nm(), vec![380.0, 450.0, 550.0, 700.0]);
    }

    #[test]
    fn wavelength_axis_range_nm_variant() {
        let axis = WavelengthAxis {
            values_nm: None,
            range_nm: Some(WavelengthRange {
                start: 380.0,
                end: 400.0,
                interval: 10.0,
            }),
        };
        let wls = axis.wavelengths_nm();
        assert_eq!(wls.len(), 3);
        assert!((wls[0] - 380.0).abs() < 1e-10);
        assert!((wls[1] - 390.0).abs() < 1e-10);
        assert!((wls[2] - 400.0).abs() < 1e-10);
    }

    #[test]
    fn wavelength_range_expand_direct() {
        let r = WavelengthRange {
            start: 380.0,
            end: 780.0,
            interval: 10.0,
        };
        let wls = r.expand();
        assert_eq!(wls.len(), 41);
        assert!((wls[0] - 380.0).abs() < 1e-10);
        assert!((wls[40] - 780.0).abs() < 1e-10);
    }

    // ── Cross-field validation edge cases ─────────────────────────────────────

    #[test]
    fn uncertainty_length_mismatch_is_error() {
        let json = r#"{
            "schema_version": "1.0.0",
            "file_type": "single",
            "spectrum": {
                "id": "x",
                "metadata": {"measurement_type": "reflectance", "date": "2026-04-29"},
                "wavelength_axis": {"values_nm": [380, 390, 400]},
                "spectral_data": {"values": [0.1, 0.2, 0.3], "uncertainty": [0.01, 0.01]}
            }
        }"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::CrossFieldValidation(_))
        ));
    }

    #[test]
    fn illuminant_custom_sd_length_mismatch_is_error() {
        let json = r#"{
            "schema_version": "1.0.0",
            "file_type": "single",
            "spectrum": {
                "id": "x",
                "metadata": {"measurement_type": "reflectance", "date": "2026-04-29"},
                "wavelength_axis": {"values_nm": [380, 390, 400]},
                "spectral_data": {"values": [0.1, 0.2, 0.3]},
                "color_science": {
                    "illuminant": "custom",
                    "illuminant_custom_sd": {
                        "wavelengths_nm": [380, 390, 400],
                        "values": [1.0, 1.1]
                    }
                }
            }
        }"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::CrossFieldValidation(_))
        ));
    }

    // ── from_path and from_str_unchecked ──────────────────────────────────────

    #[test]
    fn from_path_loads_single_example() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/example_single.json");
        let file = SpectrumFile::from_path(path).unwrap();
        assert_eq!(file.spectra().len(), 1);
        assert_eq!(file.spectra()[0].id, "sample-001");
    }

    #[test]
    fn from_path_loads_batch_example() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/example_batch.json");
        let file = SpectrumFile::from_path(path).unwrap();
        assert_eq!(file.spectra().len(), 2);
    }

    #[test]
    fn from_str_unchecked_skips_cross_field_validation() {
        // 3 wavelengths but only 2 values — cross-field check rejects this, unchecked accepts it
        let json = r#"{
            "schema_version": "1.0.0",
            "file_type": "single",
            "spectrum": {
                "id": "x",
                "metadata": {"measurement_type": "reflectance", "date": "2026-04-29"},
                "wavelength_axis": {"values_nm": [380, 390, 400]},
                "spectral_data": {"values": [0.1, 0.2]}
            }
        }"#;
        assert!(SpectrumFile::from_str_unchecked(json).is_ok());
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::CrossFieldValidation(_))
        ));
    }

    // ── batch_metadata accessor ───────────────────────────────────────────────

    #[test]
    fn batch_metadata_fields_accessible() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/example_batch.json");
        let file = SpectrumFile::from_path(path).unwrap();
        let meta = file
            .batch_metadata()
            .expect("batch file must have metadata");
        assert_eq!(
            meta.title.as_deref(),
            Some("Ceramic tile color survey - April 2026")
        );
        assert_eq!(meta.operator.as_deref(), Some("J. Smith"));
    }

    #[test]
    fn batch_metadata_returns_none_for_single_file() {
        let file = SpectrumFile::from_json_str(&make_single("reflectance", &wls_41(), &vals_41()))
            .unwrap();
        assert!(file.batch_metadata().is_none());
    }

    #[test]
    fn percent_scale_reflectance_above_one_is_ok() {
        // scale="percent" means values are 0–100; the [0,1] bounds check must not fire.
        let json = r#"{"schema_version":"1.0.0","file_type":"single","spectrum":{"id":"x",
            "metadata":{"measurement_type":"reflectance","date":"2026-04-29"},
            "wavelength_axis":{"values_nm":[380,390,400]},
            "spectral_data":{"values":[50.0,75.0,85.0],"scale":"percent"}}}"#;
        assert!(SpectrumFile::from_json_str(json).is_ok());
    }

    #[test]
    fn single_file_with_spectra_key_is_schema_error() {
        let json = r#"{"schema_version":"1.0.0","file_type":"single",
            "spectrum":{"id":"x","metadata":{"measurement_type":"reflectance","date":"2026-04-29"},
            "wavelength_axis":{"values_nm":[380,390]},"spectral_data":{"values":[0.1,0.2]}},
            "spectra":[]}"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::SchemaValidation(_))
        ));
    }

    #[test]
    fn empty_spectra_array_is_schema_error() {
        let json = r#"{"schema_version":"1.0.0","file_type":"batch","spectra":[]}"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::SchemaValidation(_))
        ));
    }

    #[test]
    fn invalid_illuminant_is_schema_error() {
        let json = r#"{"schema_version":"1.0.0","file_type":"single","spectrum":{"id":"x",
            "metadata":{"measurement_type":"reflectance","date":"2026-04-29"},
            "wavelength_axis":{"values_nm":[380,390,400]},
            "spectral_data":{"values":[0.1,0.2,0.3]},
            "color_science":{"illuminant":"TL84"}}}"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::SchemaValidation(_))
        ));
    }

    #[test]
    fn invalid_cie_observer_is_schema_error() {
        let json = r#"{"schema_version":"1.0.0","file_type":"single","spectrum":{"id":"x",
            "metadata":{"measurement_type":"reflectance","date":"2026-04-29"},
            "wavelength_axis":{"values_nm":[380,390,400]},
            "spectral_data":{"values":[0.1,0.2,0.3]},
            "color_science":{"cie_observer":"CIE 2006"}}}"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::SchemaValidation(_))
        ));
    }

    #[test]
    fn values_nm_fewer_than_two_is_schema_error() {
        let json = r#"{"schema_version":"1.0.0","file_type":"single","spectrum":{"id":"x",
            "metadata":{"measurement_type":"reflectance","date":"2026-04-29"},
            "wavelength_axis":{"values_nm":[380]},
            "spectral_data":{"values":[0.1]}}}"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::SchemaValidation(_))
        ));
    }

    #[test]
    fn range_nm_non_positive_interval_is_schema_error() {
        let json = r#"{"schema_version":"1.0.0","file_type":"single","spectrum":{"id":"x",
            "metadata":{"measurement_type":"reflectance","date":"2026-04-29"},
            "wavelength_axis":{"range_nm":{"start":380,"end":780,"interval":0}},
            "spectral_data":{"values":[0.1,0.2]}}}"#;
        assert!(matches!(
            SpectrumFile::from_json_str(json),
            Err(SpectrumFileError::SchemaValidation(_))
        ));
    }

    // Conversion-to-Spectrum tests live in the colorimetry crate.
}
