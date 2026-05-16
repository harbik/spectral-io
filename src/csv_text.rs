//! Generic CSV / TSV importer and exporter for spectral data.
//!
//! Files have two sections:
//!
//! 1. **Header block** — key/value lines using `KEY: VALUE` or `KEY = VALUE`
//!    syntax, or `KEY<delim>VALUE` for a set of recognised metadata keywords.
//!    Lines starting with `#` and blank lines are ignored throughout.
//!
//! 2. **Data block** — started by the first row whose first cell parses as a
//!    number (the wavelength in nm).  The immediately preceding non-blank,
//!    non-comment line (if its first cell is not a number) is the optional
//!    column-header row.  First column = wavelength; remaining columns = one
//!    spectrum each.
//!
//! [`csv_write`] always emits `KEY: VALUE` metadata lines so round-trips are
//! unambiguous regardless of the delimiter chosen on export.

use super::*;

// Keys accepted as two-field delimiter-separated metadata pairs without an
// explicit `: ` or ` = ` separator.
const KNOWN_META_KEYS: &[&str] = &[
    "TITLE",
    "NAME",
    "SAMPLE_NAME",
    "DATE",
    "CREATED",
    "MEASUREMENT_TYPE",
    "SPECTRUM_TYPE",
    "TYPE",
    "OPERATOR",
    "ORIGINATOR",
    "INSTRUMENT",
    "INSTRUMENTATION",
    "DESCRIPTION",
    "FILE_DESCRIPTOR",
    "COPYRIGHT",
    "SURFACE",
    "SAMPLE_BACKING",
    "ILLUMINANT",
    "OBSERVER",
    "SAMPLE_ID",
    "NOTES",
    "NOTE",
    "SOFTWARE",
    "SOURCE",
    "GEOMETRY",
];

// ─────────────────────────────────────────────────────────────────────────────
// Public entry points (called from lib.rs)
// ─────────────────────────────────────────────────────────────────────────────

pub(super) fn csv_parse(input: &str, source_file: Option<&str>) -> Result<SpectrumFile> {
    let meaningful: Vec<&str> = input
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .collect();

    if meaningful.is_empty() {
        return Err(SpectrumFileError::SchemaValidation(
            "CSV/TSV: file contains no data".into(),
        ));
    }

    let delimiter = detect_delimiter(&meaningful);

    let data_start = meaningful
        .iter()
        .position(|l| first_field(l, delimiter).parse::<f64>().is_ok())
        .ok_or_else(|| {
            SpectrumFileError::SchemaValidation(
                "CSV/TSV: no data rows found (no line whose first cell is a number)".into(),
            )
        })?;

    let pre_data = &meaningful[..data_start];
    let data_lines = &meaningful[data_start..];

    let (meta, col_headers) = parse_pre_data(pre_data, delimiter);
    let (wavelengths, columns) = parse_data(data_lines, delimiter)?;

    if wavelengths.len() < 2 {
        return Err(SpectrumFileError::SchemaValidation(
            "CSV/TSV: at least 2 wavelength rows are required".into(),
        ));
    }
    if !wavelengths.windows(2).all(|w| w[0] < w[1]) {
        return Err(SpectrumFileError::SchemaValidation(
            "CSV/TSV: wavelength values must be strictly increasing".into(),
        ));
    }
    if columns.is_empty() {
        return Err(SpectrumFileError::SchemaValidation(
            "CSV/TSV: no spectral data columns found (need at least 2 columns)".into(),
        ));
    }

    let n_cols = columns.len();
    let ids: Vec<String> = match col_headers {
        Some(headers) => {
            // headers[0] is the wavelength label; data column names start at index 1.
            let mut ids: Vec<String> = headers
                .into_iter()
                .skip(1)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            while ids.len() < n_cols {
                ids.push(format!("spectrum_{}", ids.len() + 1));
            }
            ids
        }
        None => (1..=n_cols).map(|i| format!("spectrum_{i}")).collect(),
    };

    let wavelength_axis = wavelength_axis_from_list(&wavelengths);

    let records: Vec<SpectrumRecord> = ids
        .into_iter()
        .zip(columns)
        .map(|(id, values)| build_record(id, values, &meta, &wavelength_axis, source_file))
        .collect();

    Ok(if records.len() == 1 {
        let mut v = records;
        SpectrumFile::Single {
            schema_version: "1.0.0".to_string(),
            spectrum: Box::new(v.remove(0)),
        }
    } else {
        SpectrumFile::Batch {
            schema_version: "1.0.0".to_string(),
            batch_metadata: None,
            spectra: records,
        }
    })
}

pub(super) fn csv_write(file: &SpectrumFile, delimiter: char) -> String {
    let spectra = file.spectra();
    if spectra.is_empty() {
        return String::new();
    }
    let first = spectra[0];
    let mut out = String::new();

    write_meta_header(&mut out, first);

    // Column header row: wavelength label followed by one ID per spectrum.
    let sep = delimiter.to_string();
    let header_row: String = std::iter::once("wavelength_nm".to_string())
        .chain(spectra.iter().map(|sp| quote_field(&sp.id, delimiter)))
        .collect::<Vec<_>>()
        .join(&sep);
    out.push_str(&header_row);
    out.push('\n');

    // Data rows.
    let wavelengths = first.wavelength_axis.wavelengths_nm();
    let all_values: Vec<&Vec<f64>> = spectra.iter().map(|sp| &sp.spectral_data.values).collect();
    debug_assert!(
        all_values.iter().all(|v| v.len() == wavelengths.len()),
        "csv_write: all spectra must have the same number of values as the wavelength axis"
    );
    for (i, wl) in wavelengths.iter().enumerate() {
        let mut row = format!("{wl}");
        for vals in &all_values {
            row.push(delimiter);
            if let Some(&v) = vals.get(i) {
                row.push_str(&format!("{v}"));
            }
        }
        out.push_str(&row);
        out.push('\n');
    }

    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn detect_delimiter(lines: &[&str]) -> char {
    let tabs: usize = lines.iter().map(|l| l.matches('\t').count()).sum();
    let commas: usize = lines.iter().map(|l| l.matches(',').count()).sum();
    if tabs >= commas {
        '\t'
    } else {
        ','
    }
}

fn first_field(line: &str, delimiter: char) -> &str {
    line.split(delimiter).next().unwrap_or("").trim()
}

// RFC4180-aware splitter: handles double-quote–enclosed fields and "" escapes.
fn split_fields(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match (in_quotes, c) {
            (true, '"') => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = false;
                }
            }
            (false, '"') => in_quotes = true,
            (false, c) if c == delimiter => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            (_, c) => current.push(c),
        }
    }
    fields.push(current.trim().to_string());
    fields
}

// Try to extract a KEY: VALUE or KEY = VALUE or known-keyword delimiter-kv pair.
// Returns None if the line should be treated as a column-header row.
fn try_kv(line: &str, delimiter: char) -> Option<(String, String)> {
    // KEY: VALUE  (colon + space — unambiguous regardless of delimiter)
    if let Some(pos) = line.find(": ") {
        let key = line[..pos].trim();
        let val = line[pos + 2..].trim();
        if !key.is_empty() && !val.is_empty() && !key.contains(delimiter) {
            return Some((key.to_string(), val.to_string()));
        }
    }
    // KEY = VALUE  (space + equals + space)
    if let Some(pos) = line.find(" = ") {
        let key = line[..pos].trim();
        let val = line[pos + 3..].trim();
        if !key.is_empty() && !val.is_empty() && !key.contains(delimiter) {
            return Some((key.to_string(), val.to_string()));
        }
    }
    // KEY<delim>VALUE  where KEY is a recognised metadata keyword
    let parts: Vec<&str> = line.splitn(2, delimiter).collect();
    if parts.len() == 2 {
        let key_up = parts[0].trim().to_uppercase();
        if KNOWN_META_KEYS.contains(&key_up.as_str()) {
            return Some((parts[0].trim().to_string(), parts[1].trim().to_string()));
        }
    }
    None
}

struct CsvMeta {
    title: Option<String>,
    date: String,
    measurement_type: MeasurementType,
    operator: Option<String>,
    instrument_model: Option<String>,
    description: Option<String>,
    copyright: Option<String>,
    surface: Option<String>,
    sample_backing: Option<String>,
    sample_id: Option<String>,
    illuminant: Option<String>,
    observer: Option<String>,
    notes: Option<String>,
    custom: serde_json::Map<String, serde_json::Value>,
}

impl Default for CsvMeta {
    fn default() -> Self {
        Self {
            title: None,
            date: "1970-01-01".to_string(),
            measurement_type: MeasurementType::Reflectance,
            operator: None,
            instrument_model: None,
            description: None,
            copyright: None,
            surface: None,
            sample_backing: None,
            sample_id: None,
            illuminant: None,
            observer: None,
            notes: None,
            custom: serde_json::Map::new(),
        }
    }
}

fn parse_pre_data(pre_data: &[&str], delimiter: char) -> (CsvMeta, Option<Vec<String>>) {
    let mut meta = CsvMeta::default();
    let mut col_headers: Option<Vec<String>> = None;

    for &line in pre_data {
        if let Some((key, value)) = try_kv(line, delimiter) {
            apply_kv(&key, &value, &mut meta);
        } else {
            let fields = split_fields(line, delimiter);
            if fields.len() >= 2 {
                col_headers = Some(fields);
            }
        }
    }
    (meta, col_headers)
}

fn apply_kv(key: &str, value: &str, meta: &mut CsvMeta) {
    match key.to_lowercase().as_str() {
        "title" | "name" | "sample_name" => meta.title = Some(value.to_string()),
        "date" | "created" => {
            if let Some(d) = parse_date(value) {
                meta.date = d;
            }
        }
        "measurement_type" | "spectrum_type" | "type" => {
            meta.measurement_type = parse_measurement_type(value);
        }
        "operator" | "originator" => meta.operator = Some(value.to_string()),
        "instrument" | "instrumentation" => meta.instrument_model = Some(value.to_string()),
        "description" | "file_descriptor" => meta.description = Some(value.to_string()),
        "copyright" => meta.copyright = Some(value.to_string()),
        "surface" => meta.surface = Some(value.to_string()),
        "sample_backing" => meta.sample_backing = Some(value.to_string()),
        "sample_id" => meta.sample_id = Some(value.to_string()),
        "illuminant" => meta.illuminant = Some(value.to_string()),
        "observer" => meta.observer = Some(value.to_string()),
        "notes" | "note" => meta.notes = Some(value.to_string()),
        _ => {
            meta.custom.insert(
                key.to_lowercase(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
}

fn parse_data(data_lines: &[&str], delimiter: char) -> Result<(Vec<f64>, Vec<Vec<f64>>)> {
    let mut wavelengths: Vec<f64> = Vec::new();
    let mut columns: Vec<Vec<f64>> = Vec::new();
    let mut n_cols: Option<usize> = None;

    for &line in data_lines {
        let fields = split_fields(line, delimiter);
        if fields.is_empty() {
            continue;
        }
        let wl: f64 = match fields[0].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let n = fields.len() - 1;
        if n_cols.is_none() {
            n_cols = Some(n);
            columns = vec![Vec::new(); n];
        }
        let expected = n_cols.unwrap();
        wavelengths.push(wl);
        for i in 0..expected {
            let v: f64 = if i + 1 < fields.len() {
                fields[i + 1].parse().unwrap_or(f64::NAN)
            } else {
                f64::NAN
            };
            columns[i].push(v);
        }
    }

    Ok((wavelengths, columns))
}

fn build_record(
    id: String,
    values: Vec<f64>,
    meta: &CsvMeta,
    wavelength_axis: &WavelengthAxis,
    source_file: Option<&str>,
) -> SpectrumRecord {
    let instrument = meta.instrument_model.as_ref().map(|m| Instrument {
        manufacturer: None,
        model: Some(m.clone()),
        serial_number: None,
        detector_type: None,
        light_source: None,
    });

    let illuminant = meta.illuminant.as_deref().and_then(|s| {
        let up = s.to_uppercase();
        ALLOWED_ILLUMINANTS.contains(&up.as_str()).then_some(up)
    });
    let observer = meta.observer.as_deref().and_then(parse_observer);
    let color_science = (illuminant.is_some() || observer.is_some()).then_some(ColorScience {
        illuminant,
        illuminant_custom_sd: None,
        cie_observer: observer,
        white_reference: None,
        results: None,
    });

    let custom = (!meta.custom.is_empty()).then(|| serde_json::Value::Object(meta.custom.clone()));

    SpectrumRecord {
        id,
        metadata: SpectrumMetadata {
            measurement_type: meta.measurement_type,
            date: meta.date.clone(),
            title: meta.title.clone(),
            description: meta.description.clone(),
            sample_id: meta.sample_id.clone(),
            time: None,
            operator: meta.operator.clone(),
            instrument,
            measurement_conditions: None,
            surface: meta.surface.clone(),
            sample_backing: meta.sample_backing.clone(),
            tags: None,
            copyright: meta.copyright.clone(),
            custom,
        },
        wavelength_axis: wavelength_axis.clone(),
        spectral_data: SpectralData {
            values,
            uncertainty: None,
            scale: None,
        },
        color_science,
        provenance: Some(Provenance {
            software: None,
            software_version: None,
            source_file: source_file.map(str::to_string),
            source_format: Some("CSV/TSV".into()),
            processing_steps: None,
            notes: meta.notes.clone(),
        }),
    }
}

fn write_meta_header(out: &mut String, sp: &SpectrumRecord) {
    let m = &sp.metadata;
    out.push_str(&format!(
        "Measurement_Type: {}\n",
        measurement_type_str(m.measurement_type)
    ));
    out.push_str(&format!("Date: {}\n", m.date));
    if let Some(t) = &m.title {
        out.push_str(&format!("Title: {t}\n"));
    }
    if let Some(op) = &m.operator {
        out.push_str(&format!("Operator: {op}\n"));
    }
    if let Some(desc) = &m.description {
        out.push_str(&format!("Description: {desc}\n"));
    }
    if let Some(c) = &m.copyright {
        out.push_str(&format!("Copyright: {c}\n"));
    }
    if let Some(s) = &m.surface {
        out.push_str(&format!("Surface: {s}\n"));
    }
    if let Some(sb) = &m.sample_backing {
        out.push_str(&format!("Sample_Backing: {sb}\n"));
    }
    if let Some(inst) = &m.instrument {
        if let Some(model) = &inst.model {
            out.push_str(&format!("Instrument: {model}\n"));
        }
    }
    if let Some(sid) = &m.sample_id {
        out.push_str(&format!("Sample_ID: {sid}\n"));
    }
    if let Some(cs) = &sp.color_science {
        if let Some(ill) = &cs.illuminant {
            out.push_str(&format!("Illuminant: {ill}\n"));
        }
        if let Some(obs) = &cs.cie_observer {
            out.push_str(&format!("Observer: {obs}\n"));
        }
    }
    if let Some(notes) = sp.provenance.as_ref().and_then(|p| p.notes.as_deref()) {
        out.push_str(&format!("Notes: {notes}\n"));
    }
    out.push('\n');
}

fn quote_field(s: &str, delimiter: char) -> String {
    if s.contains(delimiter) || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub(super) fn wavelength_axis_from_list(wls: &[f64]) -> WavelengthAxis {
    if wls.len() >= 2 {
        let interval = wls[1] - wls[0];
        if interval > 0.0
            && wls
                .windows(2)
                .all(|w| (w[1] - w[0] - interval).abs() < 0.001)
        {
            return WavelengthAxis {
                values_nm: None,
                range_nm: Some(WavelengthRange {
                    start: wls[0],
                    end: *wls.last().unwrap(),
                    interval,
                }),
            };
        }
    }
    WavelengthAxis {
        values_nm: Some(wls.to_vec()),
        range_nm: None,
    }
}

fn parse_date(s: &str) -> Option<String> {
    let s = s.trim();
    // ISO 8601: YYYY-MM-DD
    if s.len() >= 10 && s.as_bytes().get(4) == Some(&b'-') && s.as_bytes().get(7) == Some(&b'-') {
        return Some(s[..10].to_string());
    }
    // MM/DD/YYYY
    let parts: Vec<&str> = s.splitn(3, '/').collect();
    if parts.len() == 3 {
        let m = parts[0].trim();
        let d = parts[1].trim();
        let y = parts[2].split_whitespace().next().unwrap_or("");
        if m.parse::<u32>().is_ok()
            && d.parse::<u32>().is_ok()
            && y.len() == 4
            && y.parse::<u32>().is_ok()
        {
            return Some(format!("{y}-{m:0>2}-{d:0>2}"));
        }
    }
    None
}

fn parse_measurement_type(s: &str) -> MeasurementType {
    let lower = s.to_lowercase();
    if lower.contains("transmit") {
        MeasurementType::Transmittance
    } else if lower.contains("absorb") {
        MeasurementType::Absorbance
    } else if lower.contains("irradiance") {
        MeasurementType::Irradiance
    } else if lower.contains("radiance") {
        MeasurementType::Radiance
    } else if lower.contains("emission") || lower.contains("emissive") {
        MeasurementType::Emission
    } else {
        MeasurementType::Reflectance
    }
}

fn parse_observer(s: &str) -> Option<String> {
    if s.contains("10") {
        Some("CIE 1964 10 degree".to_string())
    } else if s.contains('2') {
        Some("CIE 1931 2 degree".to_string())
    } else {
        None
    }
}

fn measurement_type_str(mt: MeasurementType) -> &'static str {
    match mt {
        MeasurementType::Reflectance => "reflectance",
        MeasurementType::Transmittance => "transmittance",
        MeasurementType::Absorbance => "absorbance",
        MeasurementType::Radiance => "radiance",
        MeasurementType::Irradiance => "irradiance",
        MeasurementType::Emission => "emission",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const TSV_SINGLE: &str = "Measurement_Type: reflectance\n\
        Date: 2026-05-15\n\
        wavelength_nm\tsample_a\n\
        380\t0.041\n\
        390\t0.052\n\
        400\t0.063\n";

    const TSV_BATCH: &str = "Measurement_Type: reflectance\n\
        Date: 2026-05-15\n\
        wavelength_nm\tA\tB\n\
        380\t0.041\t0.089\n\
        390\t0.052\t0.092\n\
        400\t0.063\t0.095\n";

    #[test]
    fn single_column_single_spectrum() {
        let file = csv_parse(TSV_SINGLE, None).unwrap();
        let spectra = file.spectra();
        assert_eq!(spectra.len(), 1);
        assert_eq!(spectra[0].id, "sample_a");
        assert_eq!(spectra[0].n_points(), 3);
        assert_eq!(spectra[0].points()[0], (380.0, 0.041));
        assert!(matches!(file, SpectrumFile::Single { .. }));
    }

    #[test]
    fn multi_column_batch() {
        let file = csv_parse(TSV_BATCH, None).unwrap();
        let spectra = file.spectra();
        assert_eq!(spectra.len(), 2);
        assert_eq!(spectra[0].id, "A");
        assert_eq!(spectra[1].id, "B");
        assert!(matches!(file, SpectrumFile::Batch { .. }));
    }

    #[test]
    fn no_column_headers_auto_id() {
        let input = "Measurement_Type: reflectance\nDate: 2026-05-15\n380\t0.041\n390\t0.052\n";
        let file = csv_parse(input, None).unwrap();
        assert_eq!(file.spectra()[0].id, "spectrum_1");
    }

    #[test]
    fn metadata_parsed() {
        let input = "Measurement_Type: transmittance\n\
            Date: 2026-05-15\n\
            Title: My Filter\n\
            Illuminant: D65\n\
            Observer: CIE 1931 2 degree\n\
            wavelength_nm\tfilter\n\
            500\t0.5\n\
            510\t0.6\n";
        let file = csv_parse(input, None).unwrap();
        let sp = file.spectra()[0];
        assert!(matches!(
            sp.metadata.measurement_type,
            MeasurementType::Transmittance
        ));
        assert_eq!(sp.metadata.title.as_deref(), Some("My Filter"));
        assert_eq!(sp.metadata.date, "2026-05-15");
        let cs = sp.color_science.as_ref().unwrap();
        assert_eq!(cs.illuminant.as_deref(), Some("D65"));
        assert_eq!(cs.cie_observer.as_deref(), Some("CIE 1931 2 degree"));
    }

    #[test]
    fn regular_grid_detected() {
        let file = csv_parse(TSV_SINGLE, None).unwrap();
        let rng = file.spectra()[0].wavelength_axis.range_nm.as_ref().unwrap();
        assert_eq!(rng.start, 380.0);
        assert_eq!(rng.end, 400.0);
        assert_eq!(rng.interval, 10.0);
    }

    #[test]
    fn irregular_grid_uses_values_nm() {
        let input = "Date: 2026-05-15\n\
            Measurement_Type: reflectance\n\
            wavelength_nm\ts\n\
            380\t0.1\n\
            385\t0.2\n\
            400\t0.3\n";
        let file = csv_parse(input, None).unwrap();
        let sp = file.spectra()[0];
        assert!(sp.wavelength_axis.values_nm.is_some());
        assert!(sp.wavelength_axis.range_nm.is_none());
    }

    #[test]
    fn csv_comma_delimiter() {
        let input = "Measurement_Type: reflectance\n\
            Date: 2026-05-15\n\
            wavelength_nm,A,B\n\
            380,0.041,0.089\n\
            390,0.052,0.092\n";
        let file = csv_parse(input, None).unwrap();
        let spectra = file.spectra();
        assert_eq!(spectra.len(), 2);
        assert_eq!(spectra[0].id, "A");
        assert_eq!(spectra[1].id, "B");
    }

    #[test]
    fn optional_blank_line_between_header_and_data() {
        let input = "Measurement_Type: reflectance\nDate: 2026-05-15\n\nwavelength_nm\tA\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        assert_eq!(file.spectra().len(), 1);
    }

    #[test]
    fn round_trip_tsv() {
        let file1 = csv_parse(TSV_BATCH, None).unwrap();
        let tsv = csv_write(&file1, '\t');
        let file2 = csv_parse(&tsv, None).unwrap();
        let s1 = file1.spectra();
        let s2 = file2.spectra();
        assert_eq!(s1.len(), s2.len());
        assert_eq!(s1[0].id, s2[0].id);
        assert_eq!(s1[1].id, s2[1].id);
        assert_eq!(s1[0].points(), s2[0].points());
    }

    #[test]
    fn round_trip_csv() {
        let file1 = csv_parse(TSV_BATCH, None).unwrap();
        let csv = csv_write(&file1, ',');
        let file2 = csv_parse(&csv, None).unwrap();
        assert_eq!(file1.spectra().len(), file2.spectra().len());
        assert_eq!(file1.spectra()[0].id, file2.spectra()[0].id);
    }

    #[test]
    fn error_on_no_data() {
        assert!(csv_parse("# just a comment\n", None).is_err());
    }

    #[test]
    fn error_on_no_numeric_rows() {
        assert!(csv_parse("Title: foo\nwavelength_nm\tA\n", None).is_err());
    }

    #[test]
    fn error_on_single_wavelength_row() {
        assert!(csv_parse(
            "Date: 2026-01-01\nMeasurement_Type: reflectance\nw\ts\n380\t0.1\n",
            None
        )
        .is_err());
    }

    #[test]
    fn error_on_no_data_columns() {
        // Wavelength column only — no spectral data columns.
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n380\n390\n";
        assert!(csv_parse(input, None).is_err());
    }

    // ── try_kv: KEY = VALUE separator ────────────────────────────────────────

    #[test]
    fn equals_separator_parsed_as_metadata() {
        let input = "Measurement_Type = reflectance\nDate = 2026-05-15\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        let sp = file.spectra()[0];
        assert!(matches!(
            sp.metadata.measurement_type,
            MeasurementType::Reflectance
        ));
        assert_eq!(sp.metadata.date, "2026-05-15");
    }

    // ── try_kv: known-keyword tab-delimited pair ──────────────────────────────

    #[test]
    fn known_key_tab_pair_parsed_as_metadata() {
        // "TITLE\tMy Sample" has no `: ` or ` = ` but TITLE is a known key.
        let input = "TITLE\tMy Sample\nDate: 2026-05-15\nMeasurement_Type: reflectance\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        assert_eq!(
            file.spectra()[0].metadata.title.as_deref(),
            Some("My Sample")
        );
    }

    // ── apply_kv: untested metadata fields ───────────────────────────────────

    #[test]
    fn metadata_operator_and_instrument() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Operator: Alice\nInstrument: i1Pro 3\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        let sp = file.spectra()[0];
        assert_eq!(sp.metadata.operator.as_deref(), Some("Alice"));
        let inst = sp.metadata.instrument.as_ref().unwrap();
        assert_eq!(inst.model.as_deref(), Some("i1Pro 3"));
    }

    #[test]
    fn metadata_description_copyright_surface_backing() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Description: Test batch\nCopyright: © 2026 Lab\n\
            Surface: Matte\nSample_Backing: Black\n\
            wavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        let sp = file.spectra()[0];
        assert_eq!(sp.metadata.description.as_deref(), Some("Test batch"));
        assert_eq!(sp.metadata.copyright.as_deref(), Some("© 2026 Lab"));
        assert_eq!(sp.metadata.surface.as_deref(), Some("Matte"));
        assert_eq!(sp.metadata.sample_backing.as_deref(), Some("Black"));
    }

    #[test]
    fn metadata_notes_in_provenance() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Notes: measured twice\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        let sp = file.spectra()[0];
        assert_eq!(
            sp.provenance.as_ref().unwrap().notes.as_deref(),
            Some("measured twice")
        );
    }

    #[test]
    fn unknown_key_goes_to_custom() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Batch_ID: B001\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        let sp = file.spectra()[0];
        let custom = sp.metadata.custom.as_ref().unwrap();
        assert_eq!(
            custom.get("batch_id").and_then(|v| v.as_str()),
            Some("B001")
        );
    }

    // ── parse_measurement_type variants ──────────────────────────────────────

    #[test]
    fn measurement_type_absorbance() {
        let input = "Measurement_Type: absorbance\nDate: 2026-05-15\nw\ts\n380\t0.5\n390\t0.6\n";
        let file = csv_parse(input, None).unwrap();
        assert!(matches!(
            file.spectra()[0].metadata.measurement_type,
            MeasurementType::Absorbance
        ));
    }

    #[test]
    fn measurement_type_irradiance() {
        let input = "Measurement_Type: irradiance\nDate: 2026-05-15\nw\ts\n380\t1.0\n390\t1.1\n";
        let file = csv_parse(input, None).unwrap();
        assert!(matches!(
            file.spectra()[0].metadata.measurement_type,
            MeasurementType::Irradiance
        ));
    }

    #[test]
    fn measurement_type_radiance() {
        let input = "Measurement_Type: radiance\nDate: 2026-05-15\nw\ts\n380\t1.0\n390\t1.1\n";
        let file = csv_parse(input, None).unwrap();
        assert!(matches!(
            file.spectra()[0].metadata.measurement_type,
            MeasurementType::Radiance
        ));
    }

    #[test]
    fn measurement_type_emission() {
        let input = "Measurement_Type: emission\nDate: 2026-05-15\nw\ts\n380\t1.0\n390\t1.1\n";
        let file = csv_parse(input, None).unwrap();
        assert!(matches!(
            file.spectra()[0].metadata.measurement_type,
            MeasurementType::Emission
        ));
    }

    // ── parse_observer variants ───────────────────────────────────────────────

    #[test]
    fn observer_10_degree() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Observer: CIE 1964 10 degree\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        let cs = file.spectra()[0].color_science.as_ref().unwrap();
        assert_eq!(cs.cie_observer.as_deref(), Some("CIE 1964 10 degree"));
    }

    #[test]
    fn observer_unrecognised_yields_no_color_science() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Observer: unknown\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        // "unknown" matches neither "10" nor "2", so parse_observer returns None.
        // With no illuminant either, color_science should be absent.
        assert!(file.spectra()[0].color_science.is_none());
    }

    // ── illuminant handling ───────────────────────────────────────────────────

    #[test]
    fn unrecognised_illuminant_dropped() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Illuminant: Tungsten\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        assert!(file.spectra()[0].color_science.is_none());
    }

    #[test]
    fn illuminant_case_insensitive() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Illuminant: d65\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        let cs = file.spectra()[0].color_science.as_ref().unwrap();
        assert_eq!(cs.illuminant.as_deref(), Some("D65"));
    }

    // ── parse_date variants ───────────────────────────────────────────────────

    #[test]
    fn date_mm_dd_yyyy() {
        let input = "Date: 05/15/2026\nMeasurement_Type: reflectance\n\
            wavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        assert_eq!(file.spectra()[0].metadata.date, "2026-05-15");
    }

    #[test]
    fn invalid_date_falls_back_to_default() {
        let input = "Date: not-a-date\nMeasurement_Type: reflectance\n\
            wavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        assert_eq!(file.spectra()[0].metadata.date, "1970-01-01");
    }

    // ── write path: optional metadata fields ─────────────────────────────────

    #[test]
    fn write_optional_metadata_fields_round_trip() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Title: My Sample\nOperator: Bob\nDescription: Test run\n\
            Copyright: © Lab\nSurface: Gloss\nSample_Backing: White\n\
            Instrument: CM-700d\nIlluminant: D50\nObserver: CIE 1964 10 degree\n\
            wavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file1 = csv_parse(input, None).unwrap();
        let tsv = csv_write(&file1, '\t');
        let file2 = csv_parse(&tsv, None).unwrap();
        let sp1 = file1.spectra()[0];
        let sp2 = file2.spectra()[0];
        assert_eq!(sp1.metadata.title, sp2.metadata.title);
        assert_eq!(sp1.metadata.operator, sp2.metadata.operator);
        assert_eq!(sp1.metadata.description, sp2.metadata.description);
        assert_eq!(sp1.metadata.copyright, sp2.metadata.copyright);
        assert_eq!(sp1.metadata.surface, sp2.metadata.surface);
        assert_eq!(sp1.metadata.sample_backing, sp2.metadata.sample_backing);
        assert_eq!(
            sp1.metadata.instrument.as_ref().unwrap().model,
            sp2.metadata.instrument.as_ref().unwrap().model
        );
        assert_eq!(
            sp1.color_science.as_ref().unwrap().illuminant,
            sp2.color_science.as_ref().unwrap().illuminant
        );
        assert_eq!(
            sp1.color_science.as_ref().unwrap().cie_observer,
            sp2.color_science.as_ref().unwrap().cie_observer
        );
    }

    // ── quote_field: IDs containing the delimiter ─────────────────────────────

    #[test]
    fn csv_id_containing_comma_is_quoted() {
        // Build a file whose spectrum ID contains a comma, export as CSV, re-import.
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            wavelength_nm,patch A,patch B\n380,0.1,0.2\n390,0.3,0.4\n";
        let file1 = csv_parse(input, None).unwrap();
        // Manually rename first spectrum to include a comma.
        let csv = csv_write(&file1, ',');
        // The written CSV should quote any ID containing a comma.
        // Our IDs are "patch A" and "patch B" (no comma), so this just verifies
        // the quoting path isn't triggered here — test the branch directly.
        assert!(!quote_field("plain", ',').contains('"'));
        assert_eq!(quote_field("a,b", ','), "\"a,b\"");
        assert_eq!(quote_field("say \"hi\"", ','), "\"say \"\"hi\"\"\"");
        // Confirm the CSV produced is re-parseable.
        let file2 = csv_parse(&csv, None).unwrap();
        assert_eq!(file1.spectra().len(), file2.spectra().len());
    }

    // ── column header count shorter than data columns (auto-pad) ─────────────

    #[test]
    fn fewer_column_headers_than_data_columns_padded() {
        // Header row names only one data column but there are two.
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            wavelength_nm\tA\n\
            380\t0.1\t0.2\n\
            390\t0.3\t0.4\n";
        let file = csv_parse(input, None).unwrap();
        let spectra = file.spectra();
        assert_eq!(spectra.len(), 2);
        assert_eq!(spectra[0].id, "A");
        assert_eq!(spectra[1].id, "spectrum_2"); // padded
    }

    // ── sample_id ─────────────────────────────────────────────────────────────

    #[test]
    fn sample_id_parsed_into_metadata() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Sample_ID: SN-0042\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file = csv_parse(input, None).unwrap();
        assert_eq!(
            file.spectra()[0].metadata.sample_id.as_deref(),
            Some("SN-0042")
        );
    }

    #[test]
    fn sample_id_round_trips() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Sample_ID: SN-0042\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file1 = csv_parse(input, None).unwrap();
        let tsv = csv_write(&file1, '\t');
        let file2 = csv_parse(&tsv, None).unwrap();
        assert_eq!(
            file2.spectra()[0].metadata.sample_id.as_deref(),
            Some("SN-0042")
        );
    }

    // ── notes round-trip ─────────────────────────────────────────────────────

    #[test]
    fn notes_round_trip() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Notes: measured in triplicate\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file1 = csv_parse(input, None).unwrap();
        let tsv = csv_write(&file1, '\t');
        let file2 = csv_parse(&tsv, None).unwrap();
        assert_eq!(
            file2.spectra()[0]
                .provenance
                .as_ref()
                .unwrap()
                .notes
                .as_deref(),
            Some("measured in triplicate")
        );
    }

    // ── custom fields not written ─────────────────────────────────────────────

    #[test]
    fn custom_fields_not_written_on_export() {
        // Unknown keys are stored in metadata.custom on parse but are not
        // emitted by csv_write — callers should not rely on them surviving
        // a round-trip through the CSV format.
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            Batch_ID: B001\nwavelength_nm\ts\n380\t0.1\n390\t0.2\n";
        let file1 = csv_parse(input, None).unwrap();
        assert!(file1.spectra()[0].metadata.custom.is_some());
        let tsv = csv_write(&file1, '\t');
        assert!(
            !tsv.contains("Batch_ID"),
            "custom field should not be written"
        );
        let file2 = csv_parse(&tsv, None).unwrap();
        assert!(file2.spectra()[0].metadata.custom.is_none());
    }

    // ── quoted field round-trip ───────────────────────────────────────────────

    #[test]
    fn quoted_id_round_trips_through_csv() {
        // Build a file with an ID containing a comma; export as CSV; re-import.
        // csv_write quotes the ID; split_fields must unquote it on re-parse.
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            wavelength_nm,patch A,patch B\n380,0.1,0.2\n390,0.3,0.4\n";
        let mut file = csv_parse(input, None).unwrap();
        // Inject a comma into one ID so quoting is triggered.
        match &mut file {
            SpectrumFile::Batch { spectra, .. } => spectra[0].id = "red,green".to_string(),
            _ => panic!("expected Batch"),
        }
        let csv = csv_write(&file, ',');
        assert!(
            csv.contains("\"red,green\""),
            "ID should be quoted in output"
        );
        let file2 = csv_parse(&csv, None).unwrap();
        assert_eq!(file2.spectra()[0].id, "red,green");
        assert_eq!(file2.spectra()[1].id, "patch B");
    }

    // ── non-increasing wavelengths rejected ───────────────────────────────────

    #[test]
    fn error_on_non_increasing_wavelengths() {
        let input = "Date: 2026-05-15\nMeasurement_Type: reflectance\n\
            wavelength_nm\ts\n400\t0.1\n390\t0.2\n380\t0.3\n";
        assert!(csv_parse(input, None).is_err());
    }

    // ── source_file propagated into provenance ────────────────────────────────

    #[test]
    fn source_file_stored_in_provenance() {
        let file = csv_parse(TSV_SINGLE, Some("lab_data.tsv")).unwrap();
        let prov = file.spectra()[0].provenance.as_ref().unwrap();
        assert_eq!(prov.source_file.as_deref(), Some("lab_data.tsv"));
        assert_eq!(prov.source_format.as_deref(), Some("CSV/TSV"));
    }
}
