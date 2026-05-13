# spectral-io

<!-- cargo-rdme start -->

`spectral-io` reads, writes, and validates optical spectral data files.
It defines a compact JSON format — `spectrum_file_schema.json` v1.0.0 — for
UV-Vis and visible-range measurements, designed to be suitable for color-science
calculations, long-term archiving, and data exchange between instruments,
pipelines, and applications.

The format captures everything a downstream calculation needs in one place:
the measured spectrum, the physical conditions under which it was taken
(instrument, geometry, illuminant, observer), and an optional provenance trail.
The crate also ships an importer for the
[SpectraShop](https://www.chromaxion.com/) tab-separated text format, which
holds the [Chromaxion Spectral Library](https://www.chromaxion.com/spectral-library.php) —
one of the largest freely available collections of measured spectra.

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

#### Importing a SpectraShop file

```rust
use spectral_io::SpectrumFile;

let file = SpectrumFile::from_spectrashop_path("Munsell Matte 1994.txt")
    .expect("could not parse SpectraShop file");
println!("{} spectra imported", file.spectra().len());
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
| `spectrashop` | yes | Enables [`SpectrumFile::from_spectrashop_path`] and [`SpectrumFile::from_spectrashop_str`] |

### Error handling

All fallible entry points return `Result<_, `[`SpectrumFileError`]`>`.
[`SpectrumFileError`] has four variants:

- **`Io`** — file not found or unreadable.
- **`Json`** — not valid JSON.
- **`SchemaValidation`** — structural problems: missing required fields, wrong
  types, unknown enum values. All errors for the whole file are collected and
  returned together so you see every problem at once, not just the first.
- **`CrossFieldValidation`** — inter-field constraint failures: wavelength/value
  array length mismatches, non-monotonic wavelengths, reflectance values outside
  `[0, 1]`, a `"custom"` illuminant without its spectral power distribution, etc.
  Again all errors are collected before returning.

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
chart, a paint swatch book, or a series of time-series measurements. An
optional `"batch_metadata"` block carries fields common to the whole set
(title, operator, instrument, measurement conditions) so they do not need
to be repeated on every spectrum:

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
| `measurement_type` | string enum | `reflectance`, `transmittance`, `absorbance`, `radiance`, `irradiance` |
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

##### `wavelength_axis` (required)

Exactly one of `values_nm` or `range_nm` must be present — not both, not
neither. Use `range_nm` for the common case of an evenly-spaced grid (e.g.
380–780 nm at 10 nm steps); it is more compact and unambiguous. Use
`values_nm` when the instrument produces an irregular grid or when the
spacing is not constant.

| Field | Type | Notes |
|---|---|---|
| `values_nm` | number[] | explicit wavelength list in nm; min 2 entries, strictly increasing |
| `range_nm` | object | evenly-spaced grid: `start`, `end`, and `interval` (all in nm) |

##### `spectral_data` (required)

The measured values, one per wavelength point. For reflectance and
transmittance, values must lie in `[0, 1]` when `scale` is `"fractional"`
(the default), or in `[0, 100]` when `scale` is `"percent"`. There is no
range constraint for `absorbance`, `radiance`, or `irradiance`.

| Field | Type | Notes |
|---|---|---|
| `values` | number[] | measured values, one per wavelength point |
| `uncertainty` | number[] | optional per-point 1-σ uncertainty, same length as `values` |
| `scale` | string enum | `"fractional"` (0–1, default) or `"percent"` (0–100) |

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
format, averaged, smoothed, or trimmed. Fields: `software`,
`software_version`, `source_file`, `source_format`, `notes`, and an ordered
`processing_steps` array (each step has a `step` name, `description`, and
optional `parameters` object).

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

### Validation

[`SpectrumFile::from_path`] and [`SpectrumFile::from_json_str`] run two validation
passes before returning:

1. **Schema validation** — checks required fields, correct types, and that
   enum fields (`measurement_type`, `illuminant`, `cie_observer`,
   `scale`) contain only allowed values.
2. **Cross-field validation** — checks that `values_nm` and `values` have
   equal length; that `uncertainty` (if present) has the same length; that
   wavelengths are strictly increasing; that reflectance/transmittance values
   lie in `[0, 1]` when `scale` is `"fractional"`; and that a custom
   illuminant is accompanied by `illuminant_custom_sd`.

Both passes collect all errors before returning, so a single call surfaces
every problem in the file at once. Use [`SpectrumFile::from_str_unchecked`]
to skip validation entirely when the source is fully trusted.

### Importing SpectraShop files

[SpectraShop](https://www.chromaxion.com/) is measurement and colour-analysis
software by Robin Myers Imaging. Its tab-separated text export format (`.txt`)
is used to distribute the
[Chromaxion Spectral Library](https://www.chromaxion.com/spectral-library.php),
which contains measured reflectance, transmittance, and irradiance spectra for
hundreds of real-world materials — paint colours, Munsell chips, colour charts,
photographic filters, monitor primaries, fabrics, inks, and more.

Requires the `spectrashop` feature (enabled by default).
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

<!-- cargo-rdme end -->

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
