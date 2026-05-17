# spectral-io

<!-- cargo-rdme start -->

`spectral-io` reads, writes, and validates optical spectral data files.
It defines a compact JSON format — `spectrum_file_schema.json` v1.0.0 — for
UV-Vis and visible-range measurements, suitable for colour-science calculations,
long-term archiving, and data exchange between instruments, pipelines, and
applications.

The format captures everything a downstream calculation needs in one place:
the measured spectrum, the physical conditions under which it was taken
(instrument, geometry, illuminant, observer), and an optional provenance trail.
Seven `measurement_type` values are supported: `reflectance`, `transmittance`,
`absorbance`, `radiance`, `irradiance`, `emission`, and `sensitivity`.
The `sensitivity` type covers dimensionless spectral response functions such as
colour matching functions, cone fundamentals, luminous efficiency V(λ), and
alpha-opic action spectra — values for these are not constrained to any range.

The crate also supports additional file formats:

- **CSV / TSV** (`csv` feature) — generic delimited text with an optional
  `KEY: VALUE` metadata header block; import via [`SpectrumFile::from_csv_path`]
  / [`SpectrumFile::from_csv_str`], export via [`SpectrumFile::to_tsv`] /
  [`SpectrumFile::to_csv`].
- **SpectraShop** (`spectrashop` feature) — the tab-separated text format
  used to distribute the
  [Chromaxion Spectral Library](https://www.chromaxion.com/spectral-library.php),
  one of the largest freely available collections of measured spectra; import
  via [`SpectrumFile::from_spectrashop_path`] /
  [`SpectrumFile::from_spectrashop_str`].

### Quick start

#### Reading a JSON file

```rust
use spectral_io::SpectrumFile;

let file = SpectrumFile::from_path("spectrum.json").expect("could not load file");
for sp in file.spectra() {
    let (min_nm, max_nm) = sp.wavelength_range_nm().unwrap();
    println!("{}: {} points, {:.0}–{:.0} nm", sp.id, sp.n_points(), min_nm, max_nm);
}
```

#### Importing from other formats

- CSV / TSV (`csv` feature):

```rust
use spectral_io::SpectrumFile;

let file = SpectrumFile::from_csv_path("measurements.tsv")
    .expect("could not parse file");
let tsv = file.to_tsv();
```

- SpectraShop (`spectrashop` feature):

```rust
use spectral_io::SpectrumFile;

let file = SpectrumFile::from_spectrashop_path("Munsell Matte 1994.txt")
    .expect("could not parse SpectraShop file");
println!("{} spectra imported", file.spectra().len());
```

#### Resampling to a new wavelength grid

[`SpectrumRecord::resample`] converts a spectrum to any target
[`WavelengthAxis`] and appends a provenance step automatically:

```rust
use spectral_io::{SpectrumFile, ResampleMethod, WavelengthAxis, WavelengthRange};

let file = SpectrumFile::from_path("spectrum.json").unwrap();
let target = WavelengthAxis {
    range_nm: Some(WavelengthRange { start: 380.0, end: 780.0, interval: 10.0 }),
    values_nm: None,
};
for sp in file.spectra() {
    let resampled = sp.resample(&target, ResampleMethod::BoxcarAverage);
    println!("{}: {} points", resampled.id, resampled.n_points());
}
```

#### Serialising back to JSON

Any [`SpectrumFile`] can be round-tripped through `serde_json`:

```rust
let json = serde_json::to_string_pretty(&file).expect("serialisation failed");
std::fs::write("output.json", json).unwrap();
```

### Cargo features

| Feature | Default | Description |
|---|---|---|
| `spectrashop` | no | Enables [`SpectrumFile::from_spectrashop_path`] and [`SpectrumFile::from_spectrashop_str`] |
| `csv` | no | Enables [`SpectrumFile::from_csv_path`], [`SpectrumFile::from_csv_str`], [`SpectrumFile::to_tsv`], [`SpectrumFile::to_csv`], [`SpectrumFile::write_tsv`], and [`SpectrumFile::write_csv`] |
| `python` | no | Builds a [maturin](https://github.com/PyO3/maturin) Python extension module (`spectral_io`) exposing `load()`, `load_json()`, and `SpectrumFile.to_numpy()` |

### Error handling

All fallible entry points return `Result<_, `[`SpectrumFileError`]`>`.
[`SpectrumFileError`] has four variants:

- **`Io`** — file not found or unreadable.
- **`Json`** — not valid JSON.
- **`SchemaValidation`** — structural problems: missing required fields, wrong
  types, unknown enum values. All errors for the whole file are collected and
  returned together so you see every problem at once, not just the first.
- **`CrossFieldValidation`** — inter-field constraint failures: wavelength/value
  array length mismatches, non-monotonic wavelengths, reflectance/transmittance
  values outside `[0, 1]`, a `"custom"` illuminant without its spectral power
  distribution, etc. All errors are collected before returning.

Use [`SpectrumFile::from_str_unchecked`] to bypass both validation passes when
you are certain the source is well-formed (e.g. data you just wrote yourself).

### File format

Files are JSON objects with a `schema_version` (semver string) and a
`file_type` of either `"single"` or `"batch"`.

#### Single file

Use a single file when the measurement session produced exactly one spectrum —
for example a single colour patch or a one-off transmission measurement.
The spectrum lives directly under the key `"spectrum"`:

```json
{
  "schema_version": "1.0.0",
  "file_type": "single",
  "spectrum": { "id": "patch-01", "metadata": { "..." }, "..." }
}
```

#### Batch file

Use a batch file when multiple spectra share common conditions — a colour
chart, a paint swatch book, a series of colour matching functions, or a set of
time-series measurements. An optional `"batch_metadata"` block carries fields
common to the whole set (title, operator, instrument, measurement conditions)
so they do not need to be repeated on every spectrum:

```json
{
  "schema_version": "1.0.0",
  "file_type": "batch",
  "batch_metadata": { "title": "Munsell Matte 1994", "date": "1994-01-01" },
  "spectra": [ { "id": "5R 4/2", "..." }, { "id": "5YR 4/2", "..." } ]
}
```

#### SpectrumRecord object

Each spectrum has four top-level sections (two required, two optional).

##### `metadata` (required)

Descriptive information about what was measured and how. `measurement_type`
and `date` are the only required sub-fields; everything else is optional but
strongly encouraged for reproducibility.

| Field | Type | Notes |
|---|---|---|
| `measurement_type` | string enum | `reflectance`, `transmittance`, `absorbance`, `radiance`, `irradiance`, `emission`, `sensitivity` |
| `date` | string | ISO 8601 date (`YYYY-MM-DD`) |
| `title` | string | optional human-readable name for the sample |
| `sample_id` | string | optional machine-readable sample identifier |
| `operator` | string | optional name or ID of the person who measured |
| `instrument` | object | optional: `manufacturer`, `model`, `serial_number`, `detector_type`, `light_source` |
| `measurement_conditions` | object | optional: `integration_time_ms`, `averaging`, `temperature_celsius`, `geometry`, `specular_component`, `spectral_resolution_nm`, `measurement_aperture_mm`, `measurement_filter` |
| `surface` | string | optional surface finish of the specimen (e.g. `"Matte"`, `"Gloss"`, `"Semigloss"`) |
| `sample_backing` | string | optional backing used behind the specimen during measurement (e.g. `"Black"`, `"White"`) |
| `tags` | string[] | optional free-form labels for search and filtering |
| `copyright` | string | optional copyright notice (e.g. `"© 2024 Acme Lab"`) |
| `custom` | object | optional user-defined key/value pairs for application-specific metadata |

The `measurement_type` values divide into two groups for validation purposes:
- **Bounded**: `reflectance` and `transmittance` — values must lie in `[0, 1]`
  (`"fractional"` scale) or `[0, 100]` (`"percent"` scale).
- **Unconstrained**: `absorbance`, `radiance`, `irradiance`, `emission`, and
  `sensitivity` — no range constraint is applied.

Although steady-state `absorbance` (A = −log₁₀T) is physically ≥ 0, no lower
bound is enforced because differential absorbance (ΔA, as measured in
pump-probe / transient absorption spectroscopy) can be negative, and optical
gain media produce A < 0 by stimulated emission. Similarly, `radiance` and
`irradiance` are non-negative in isolation but can go negative in
noise-corrected or difference measurements. `sensitivity` is intended for
dimensionless spectral response functions — colour matching functions, cone
fundamentals, luminous efficiency V(λ), and alpha-opic action spectra — whose
values are not bounded by any physical limit.

##### `wavelength_axis` (required)

Exactly one of `values_nm` or `range_nm` must be present — not both, not
neither. Use `range_nm` for the common case of an evenly-spaced grid (e.g.
380–780 nm at 10 nm steps); it is more compact and unambiguous. Use
`values_nm` when the instrument produces an irregular grid or when different
spectra in a batch have different valid wavelength ranges (e.g. when a
function is undefined outside a subset of the measurement range).

| Field | Type | Notes |
|---|---|---|
| `values_nm` | number[] | explicit wavelength list in nm; min 2 entries, strictly increasing |
| `range_nm` | object | evenly-spaced grid: `start`, `end`, and `interval` (all in nm) |

##### `spectral_data` (required)

The measured values, one per wavelength point.

| Field | Type | Notes |
|---|---|---|
| `values` | number[] | measured values, one per wavelength point |
| `uncertainty` | number[] | optional per-point 1-σ uncertainty, same length as `values` |
| `scale` | string enum | `"fractional"` (0–1, default) or `"percent"` (0–100); only meaningful for reflectance/transmittance |

##### `color_science` (optional)

Metadata needed to perform CIE colorimetric calculations from the spectral
data — the illuminant under which the sample is viewed, the observer (colour
matching functions), and an optional white reference. Pre-computed colorimetric
results (`XYZ`, `Lab`, CCT, …) may also be stored here as a convenience cache;
the spectral data is always the authoritative source.

| Field | Type | Notes |
|---|---|---|
| `illuminant` | string enum | `D65`, `D50`, `D55`, `D75`, `A`, `B`, `C`, `F1`–`F12`, `LED-*`, or `"custom"` |
| `illuminant_custom_sd` | object | required when `illuminant` is `"custom"`; provide the SPD as `wavelengths_nm` and `values` arrays |
| `cie_observer` | string enum | `"CIE 1931 2 degree"` (default), `"CIE 1964 10 degree"`, `"CIE 2015 2 degree"`, `"CIE 2015 10 degree"` |
| `white_reference` | object | optional calibration tile description and spectral reflectance values |
| `results` | object | optional pre-computed colorimetric values — informational only |

Available `results` sub-fields (all optional):

| Field | Type | Notes |
|---|---|---|
| `XYZ` | `[number, number, number]` | CIE tristimulus values [X, Y, Z] |
| `xy` | `[number, number]` | CIE 1931 chromaticity coordinates [x, y] |
| `uv_prime` | `[number, number]` | CIE 1976 UCS chromaticity [u′, v′] |
| `Lab` | `[number, number, number]` | CIELAB [L\*, a\*, b\*] |
| `CCT_K` | number | Correlated colour temperature in Kelvin |
| `Duv` | number | Distance from the Planckian locus (signed, CIE 1960 UCS) |

##### `provenance` (optional)

An audit trail recording where the data came from and what has been done to
it. Particularly valuable when spectra have been converted from another
format, averaged, smoothed, or resampled. Fields: `software`,
`software_version`, `source_file`, `source_format`, `notes`, and an ordered
`processing_steps` array (each step has a `step` name, `description`, and
optional `parameters` object). [`SpectrumRecord::resample`] automatically
appends a `"resample"` processing step to this trail.

#### Full single-spectrum example

A reflectance measurement of Munsell chip 5R 4/2, measured with a
Konica Minolta CM-700d in diffuse/8° geometry, stored as a regular
380–780 nm grid at 10 nm intervals:

```json
{
  "schema_version": "1.0.0",
  "file_type": "single",
  "spectrum": {
    "id": "chip-5R-4-2",
    "metadata": {
      "measurement_type": "reflectance",
      "date": "2026-04-01",
      "title": "Munsell 5R 4/2",
      "instrument": { "manufacturer": "Konica Minolta", "model": "CM-700d" },
      "measurement_conditions": { "geometry": "d:8", "specular_component": "excluded" }
    },
    "wavelength_axis": {
      "range_nm": { "start": 380, "end": 780, "interval": 10 }
    },
    "spectral_data": {
      "values": [0.048, 0.051, 0.054, 0.058, 0.063],
      "scale": "fractional"
    },
    "color_science": {
      "illuminant": "D65",
      "cie_observer": "CIE 1931 2 degree",
      "results": {
        "XYZ": [17.35, 9.12, 1.18],
        "xy": [0.629, 0.330],
        "Lab": [36.1, 55.7, 37.2]
      }
    }
  }
}
```

### Resampling

[`SpectrumRecord::resample`] converts a spectrum onto any [`WavelengthAxis`]
using one of three methods from [`ResampleMethod`]:

- **`Linear`** — linear interpolation between adjacent input samples. Output
  wavelengths outside the input range are clamped to the nearest endpoint
  (no extrapolation). Suitable for both upsampling and downsampling; exact
  for piecewise-linear data.

- **`BoxcarAverage`** — rectangular-window averaging. For each output
  wavelength λ, all input samples within ±½ step are averaged, where step
  is the mean spacing of the target axis. Falls back to linear interpolation
  when the window contains no input samples. Best for downsampling to a
  coarser regular grid (e.g. 1 nm → 10 nm).

- **`Gaussian`** — Gaussian-kernel weighted average. Each output value is a
  normalised weighted sum of input samples, with weights `exp(−½((w−λ)/σ)²)`;
  samples beyond 3σ are excluded. The kernel FWHM is taken from
  `metadata.measurement_conditions.spectral_resolution_nm` when present,
  otherwise defaults to the mean step size of the target axis.
  σ = FWHM / 2.355. Falls back to linear interpolation when no input samples
  fall within the 3σ window. Appropriate when the instrument's optical
  resolution is known and should be matched to the output sampling.

In all cases the resampled [`SpectrumRecord`] preserves the source metadata,
colour-science block, and provenance; a `"resample"` [`ProcessingStep`] is
appended automatically. Per-point `uncertainty` values are not carried
forward — correct propagation requires knowledge of the input error correlation
structure and is left to the caller.

### Validation

[`SpectrumFile::from_path`] and [`SpectrumFile::from_json_str`] run two
validation passes before returning:

1. **Schema validation** — checks required fields, correct types, and that
   enum fields (`measurement_type`, `illuminant`, `cie_observer`, `scale`)
   contain only allowed values.
2. **Cross-field validation** — checks that `values_nm` and `values` have
   equal length; that `uncertainty` (if present) has the same length; that
   wavelengths are strictly increasing; that `reflectance`/`transmittance`
   values lie in `[0, 1]` when `scale` is `"fractional"`; and that a
   `"custom"` illuminant is accompanied by `illuminant_custom_sd`.

Both passes collect all errors before returning, so a single call surfaces
every problem in the file at once. Use [`SpectrumFile::from_str_unchecked`]
to skip validation entirely when the source is fully trusted.

### CIE spectral data

The `cie_csv_to_json` example (`csv` feature) converts raw CIE data-table CSV
files (available at <https://cie.co.at/data-tables>, CC BY-SA 4.0) to the
`spectral-io` JSON format. It handles the full CIE catalogue:

- **Illuminants** — standard illuminants A, C, D50, D55, D65, D75; HP, FL, and
  LED series (5 nm and 1 nm); ID50, ID65; daylight components; reference
  spectrum L41.
- **Colour rendering** — CRI 14 test samples, CIE 99 colour fidelity samples
  (5 nm and 1 nm), CQS 15 test samples, RYGB four-colour samples, Japanese
  skin-tone complexion sample.
- **Sensitivity functions** — CIE 1931 2° and 1964 10° colour matching
  functions; Stiles–Burch 2° and 10° LMS cone fundamentals; CIE S026
  alpha-opic action spectra (S-cone, M-cone, L-cone, rod, melanopsin);
  cone-fundamental-based XYZ and luminous efficiency (2° and 10°); photopic
  V(λ) and scotopic V′(λ); first-deviation metamerism indices.

CIE CSV files use `NaN` where a function is physically undefined (e.g. z̄₁₀
above 559 nm, s̄ above 615 nm, S-cone response outside 390–615 nm). The
converter strips NaN entries per column so each spectrum carries only its valid
wavelength range, producing a correct `range_nm` or `values_nm` axis.

The converted JSON files are published in the
[spectral-data](https://github.com/harbik/spectral-data) repository under
`spectra/cie/` (CC BY-SA 4.0). To generate them locally:

```text
cargo run --example cie_csv_to_json --features csv
```

### Importing SpectraShop files

[SpectraShop](https://www.chromaxion.com/) is measurement and colour-analysis
software by Robin Myers Imaging. Its tab-separated text export format (`.txt`)
is used to distribute the
[Chromaxion Spectral Library](https://www.chromaxion.com/spectral-library.php),
which contains measured reflectance, transmittance, and irradiance spectra for
hundreds of real-world materials — paint colours, Munsell chips, colour charts,
photographic filters, monitor primaries, fabrics, inks, and more.

Requires the `spectrashop` feature.
[`SpectrumFile::from_spectrashop_path`] and [`SpectrumFile::from_spectrashop_str`]
parse the format and convert each data record in the `BEGIN_DATA`/`END_DATA`
block into a [`SpectrumRecord`]. File-level metadata (spectrum type, illuminant,
observer, geometry, etc.) is applied to every record. A file with one record
returns [`SpectrumFile::Single`]; two or more return [`SpectrumFile::Batch`].

The `spectrashop_to_json` example binary converts a SpectraShop file to
the `spectral-io` JSON format and can optionally embed a copyright notice:

```text
cargo run --example spectrashop_to_json -- -c "© Author" input.txt output.json
```

#### Format and data licensing

The SpectraShop text format is proprietary to Robin Myers Imaging. A format
specification is published at
<https://www.chromaxion.com/spectral_library/SpectraShop_Import-Export_Format.pdf>
specifically to permit third-party readers and writers.

Spectral data files from the
[Chromaxion Spectral Library](https://www.chromaxion.com/spectral-library.php)
are subject to the following terms:

- **Personal, scientific, and teaching use** is free.
- **Redistribution** requires attribution to *Chromaxion.com* or *Robin Myers*.
- **Commercial sale** of the data in any form requires express written
  permission from Robin Myers.

All data © Robin D. Myers, all rights reserved worldwide.
Contact <robin@rmimaging.com> for commercial licensing enquiries.

### Importing and exporting CSV / TSV files

Requires the `csv` feature.

[`SpectrumFile::from_csv_path`] and [`SpectrumFile::from_csv_str`] read a
generic delimited text file. The delimiter (tab or comma) is auto-detected.
Files have two sections:

1. **Header block** — zero or more `KEY: VALUE` metadata lines (or
   `KEY = VALUE`; or `KEY<delim>VALUE` for a set of recognised keywords).
   Lines starting with `#` and blank lines are ignored throughout.
   Unrecognised keys are stored in `metadata.custom`.

2. **Data block** — the first row whose first cell parses as a number
   (wavelength in nm) starts the data block. The immediately preceding
   non-blank line (if non-numeric) is the optional column-header row.
   First column = wavelength; each further column becomes one
   [`SpectrumRecord`].

A file with one data column returns [`SpectrumFile::Single`]; two or more
return [`SpectrumFile::Batch`].

Recognised header keywords (case-insensitive):

| Keyword(s) | Maps to |
|---|---|
| `Title`, `Name`, `Sample_Name` | `metadata.title` |
| `Date`, `Created` | `metadata.date` |
| `Measurement_Type`, `Spectrum_Type`, `Type` | `metadata.measurement_type` |
| `Operator`, `Originator` | `metadata.operator` |
| `Instrument`, `Instrumentation` | `metadata.instrument.model` |
| `Description`, `File_Descriptor` | `metadata.description` |
| `Copyright` | `metadata.copyright` |
| `Surface` | `metadata.surface` |
| `Sample_Backing` | `metadata.sample_backing` |
| `Sample_ID` | `metadata.sample_id` |
| `Illuminant` | `color_science.illuminant` |
| `Observer` | `color_science.cie_observer` |
| `Notes`, `Note` | `provenance.notes` |

For `Measurement_Type`, the parser accepts common synonyms:
`reflectance`/`refl`, `transmittance`/`trans`, `absorbance`/`abs`,
`radiance`/`rad`, `irradiance`/`irrad`, `emission`/`emiss`,
`sensitivity`/`response`.

```text
Measurement_Type: sensitivity
Date: 2019-01-01
Title: CIE 1931 Colour-Matching Functions, 2° Observer
Copyright: © CIE, CC BY-SA 4.0

wavelength_nm,x_bar,y_bar,z_bar
360,0.000129900,0.000003917,0.000606100
370,0.004243000,0.000120000,0.020050000
380,0.013400000,0.000396000,0.064400000
```

[`SpectrumFile::to_tsv`] and [`SpectrumFile::to_csv`] serialise back to
tab- or comma-separated text, writing `KEY: VALUE` metadata lines so files
round-trip cleanly. [`SpectrumFile::write_tsv`] and
[`SpectrumFile::write_csv`] write directly to a file path.

### Python interface (`python` feature)

The `python` feature builds a Python extension module (`spectral_io`) using
[PyO3](https://pyo3.rs/) and [maturin](https://github.com/PyO3/maturin).
It gives Python programmers direct access to spectral data as NumPy arrays,
with no manual parsing or resampling code required.

#### Installation

You need [maturin](https://github.com/PyO3/maturin) and Python ≥ 3.9.
Install inside a virtual environment:

```text
python3 -m venv .venv
source .venv/bin/activate      # Windows: .venv\Scripts\activate
pip install maturin numpy
```

Then build and install the extension in development mode (editable, no wheel):

```text
maturin develop --features python
```

Or build a release wheel for distribution or installation elsewhere:

```text
maturin build --release --features python
pip install target/wheels/spectral_io-*.whl
```

#### API

| Function / method | Description |
|---|---|
| `spectral_io.load(path)` | Load and validate a JSON file from a path |
| `spectral_io.load_json(json_str)` | Load and validate from a JSON string |
| `SpectrumFile.ids` | List of spectrum IDs in file order |
| `SpectrumFile.to_numpy(start, end, interval, method="linear")` | Resample onto an equidistant grid; returns `(wavelengths, data)` |

`to_numpy` returns a `(wavelengths, data)` tuple of NumPy float64 arrays.
`wavelengths` is always 1-D with shape `(n,)`. `data` is 1-D `(n,)` for a
single-spectrum file and 2-D `(n, m)` for a batch file, where column `j`
corresponds to `ids[j]`. `method` must be one of `"linear"` (default),
`"boxcar_average"`, or `"gaussian"`.

#### Example

```python
import spectral_io as sio
import numpy as np

# ── Single-spectrum file ─────────────────────────────────────────────────
sf = sio.load("data/spectral-io/cie/illuminants/cie_std_illum_d65.json")
print(sf.ids)                        # ['D65']

wl, spd = sf.to_numpy(380, 780, 10)  # linear interpolation (default)
print(wl.shape, spd.shape)           # (41,) (41,)
print(f"D65 at 560 nm: {spd[18]:.4f}")

# ── Batch file ───────────────────────────────────────────────────────────
sf2 = sio.load("data/spectral-io/spectrashop/Munsell Matte 1994.json")
wl, matrix = sf2.to_numpy(380, 780, 10)
print(wl.shape, matrix.shape)        # (41,) (41, 1269)
print(sf2.ids[:3])                   # ['5R 4/2', '5R 5/2', '5R 6/2']

# ── Matrix algebra ───────────────────────────────────────────────────────
# Dot the CIE 1931 colour-matching functions onto the Munsell reflectances.
cmf_file = sio.load("data/spectral-io/cie/sensitivity/cie_1931_2deg_cmf.json")
_, cmf = cmf_file.to_numpy(380, 780, 10)   # (41, 3) — x̄, ȳ, z̄ columns
XYZ = cmf.T @ matrix                        # (3, 1269)
Y = XYZ[1]                                  # luminance factor for each chip

# ── Boxcar downsampling (1 nm → 10 nm) ──────────────────────────────────
sf3 = sio.load("high_res_measurement.json")
wl, data = sf3.to_numpy(380, 780, 10, method="boxcar_average")
```

<!-- cargo-rdme end -->

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
