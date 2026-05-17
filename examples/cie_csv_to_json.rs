//! Convert CIE data-table CSV files to the spectral-io JSON format.
//!
//! # Usage
//!
//! ```text
//! cargo run --example cie_csv_to_json --features csv [-- --input DIR --output DIR]
//! ```
//!
//! Reads raw CIE CSV files from `--input` (default `data/cie-raw/`) and writes
//! JSON files to `--output` (default `data/spectral-io/cie/`).
//!
//! # Obtaining the source files
//!
//! Download the required CSVs from <https://cie.co.at/data-tables> (CC BY-SA 4.0):
//!
//! ```sh
//! mkdir -p data/cie-raw && cd data/cie-raw
//! curl -O https://files.cie.co.at/CIE_std_illum_A_1nm.csv
//! curl -O https://files.cie.co.at/CIE_std_illum_D50.csv
//! curl -O https://files.cie.co.at/CIE_std_illum_D65.csv
//! curl -O https://files.cie.co.at/CIE_illum_C.csv
//! curl -O https://files.cie.co.at/CIE_illum_D55.csv
//! curl -O https://files.cie.co.at/CIE_illum_D75.csv
//! curl -O https://files.cie.co.at/CIE_illum_HPs.csv
//! curl -O https://files.cie.co.at/CIE_illum_FLs.csv
//! curl -O https://files.cie.co.at/CIE_illum_FLs_1nm.csv
//! curl -O https://files.cie.co.at/CIE_illum_LEDs.csv
//! curl -O https://files.cie.co.at/CIE_illum_LEDs_1nm.csv
//! curl -O https://files.cie.co.at/CIE_illum_ID50.csv
//! curl -O https://files.cie.co.at/CIE_illum_ID65.csv
//! curl -O https://files.cie.co.at/CIE_illum_Dxx_comp.csv
//! curl -O https://files.cie.co.at/CIE_srf_cri.csv
//! curl -O https://files.cie.co.at/CIE_srf_cfi.csv
//! curl -O https://files.cie.co.at/CIE_srf_cfi_1nm.csv
//! curl -O https://files.cie.co.at/CIE_srf_CQS_5nm.csv
//! curl -O https://files.cie.co.at/CIE_srf_FCI_5nm.csv
//! curl -O https://files.cie.co.at/CIE_srf_PS_5nm.csv
//! ```

use spectral_io::SpectrumFile;
use std::{env, fs, path::PathBuf, process};

// ─────────────────────────────────────────────────────────────────────────────
// Dataset catalogue
// ─────────────────────────────────────────────────────────────────────────────

struct Dataset {
    csv_file: &'static str,
    subdir: &'static str,
    json_file: &'static str,
    title: &'static str,
    mtype: &'static str,
    date: &'static str,
    source: &'static str,
    doi: &'static str,
    columns: Vec<String>,
}

fn datasets() -> Vec<Dataset> {
    let fl_cols: Vec<String> = (1u8..=12)
        .map(|i| format!("FL{i}"))
        .chain((1u8..=15).map(|i| format!("FL3.{i}")))
        .collect();
    let led_cols: Vec<String> = [
        "LED-B1", "LED-B2", "LED-B3", "LED-B4", "LED-B5", "LED-BH1", "LED-RGB1", "LED-V1", "LED-V2",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let cri_cols: Vec<String> = (1u8..=14).map(|i| format!("TCS{i:02}")).collect();
    // CFI specifies 99 samples; the published CSV may contain 99 or 100 columns.
    // We provide 99 names; any extra column gets the csv_parse fallback id.
    let cfi_cols: Vec<String> = (1u8..=99).map(|i| format!("CS{i:02}")).collect();
    let cqs_cols: Vec<String> = (1u8..=15).map(|i| format!("VS{i}")).collect();

    vec![
        // ── Standard illuminants ─────────────────────────────────────────────
        Dataset {
            csv_file: "CIE_std_illum_A_1nm.csv",
            subdir: "illuminants",
            json_file: "cie_std_illum_a.json",
            title: "CIE Standard Illuminant A",
            mtype: "irradiance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition, Equation 4.1",
            doi: "10.25039/CIE.DS.8jsxjrsn",
            columns: vec!["A".into()],
        },
        Dataset {
            csv_file: "CIE_std_illum_D50.csv",
            subdir: "illuminants",
            json_file: "cie_std_illum_d50.json",
            title: "CIE Standard Illuminant D50",
            mtype: "irradiance",
            date: "2022-01-01",
            source: "ISO/CIE 11664-2:2022 Colorimetry — Part 2: CIE Standard Illuminants, Table B.1",
            doi: "10.25039/CIE.DS.etgmuqt5",
            columns: vec!["D50".into()],
        },
        Dataset {
            csv_file: "CIE_std_illum_D65.csv",
            subdir: "illuminants",
            json_file: "cie_std_illum_d65.json",
            title: "CIE Standard Illuminant D65",
            mtype: "irradiance",
            date: "2022-01-01",
            source: "ISO/CIE 11664-2:2022 Colorimetry — Part 2: CIE Standard Illuminants, Table B.1",
            doi: "10.25039/CIE.DS.hjfjmt59",
            columns: vec!["D65".into()],
        },
        Dataset {
            csv_file: "CIE_illum_C.csv",
            subdir: "illuminants",
            json_file: "cie_std_illum_c.json",
            title: "CIE Standard Illuminant C",
            mtype: "irradiance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition, Table 5",
            doi: "10.25039/CIE.DS.mjdd2enu",
            columns: vec!["C".into()],
        },
        Dataset {
            csv_file: "CIE_illum_D55.csv",
            subdir: "illuminants",
            json_file: "cie_std_illum_d55.json",
            title: "CIE Standard Illuminant D55",
            mtype: "irradiance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition, Table 5",
            doi: "10.25039/CIE.DS.qewfb3kp",
            columns: vec!["D55".into()],
        },
        Dataset {
            csv_file: "CIE_illum_D75.csv",
            subdir: "illuminants",
            json_file: "cie_std_illum_d75.json",
            title: "CIE Standard Illuminant D75",
            mtype: "irradiance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition, Table 5",
            doi: "10.25039/CIE.DS.9fvcmrk4",
            columns: vec!["D75".into()],
        },
        // ── Discharge and gas-discharge lamps ────────────────────────────────
        Dataset {
            csv_file: "CIE_illum_HPs.csv",
            subdir: "illuminants",
            json_file: "cie_illum_hp_lamps.json",
            title: "CIE High-Pressure Discharge Lamp Illuminants HP1–HP5",
            mtype: "irradiance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition, Table 11",
            doi: "10.25039/CIE.DS.f6rvvnev",
            columns: vec!["HP1".into(), "HP2".into(), "HP3".into(), "HP4".into(), "HP5".into()],
        },
        // ── Fluorescent lamp illuminants ─────────────────────────────────────
        Dataset {
            csv_file: "CIE_illum_FLs.csv",
            subdir: "illuminants",
            json_file: "cie_illum_fl_lamps_5nm.json",
            title: "CIE Fluorescent Lamp Illuminants FL1–FL12 and FL3.1–FL3.15 (5 nm)",
            mtype: "irradiance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition, Tables 10.1–10.3",
            doi: "10.25039/CIE.DS.vgssnyfg",
            columns: fl_cols.clone(),
        },
        Dataset {
            csv_file: "CIE_illum_FLs_1nm.csv",
            subdir: "illuminants",
            json_file: "cie_illum_fl_lamps_1nm.json",
            title: "CIE Fluorescent Lamp Illuminants FL1–FL12 and FL3.1–FL3.15 (1 nm)",
            mtype: "irradiance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition, Tables 10.1–10.3",
            doi: "10.25039/CIE.DS.54hy6srn",
            columns: fl_cols,
        },
        // ── LED illuminants ──────────────────────────────────────────────────
        Dataset {
            csv_file: "CIE_illum_LEDs.csv",
            subdir: "illuminants",
            json_file: "cie_illum_led_lamps_5nm.json",
            title: "CIE LED Illuminants LED-B1–B5, LED-BH1, LED-RGB1, LED-V1–V2 (5 nm)",
            mtype: "irradiance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition, Table 12",
            doi: "10.25039/CIE.DS.vgssnyfg",
            columns: led_cols.clone(),
        },
        Dataset {
            csv_file: "CIE_illum_LEDs_1nm.csv",
            subdir: "illuminants",
            json_file: "cie_illum_led_lamps_1nm.json",
            title: "CIE LED Illuminants LED-B1–B5, LED-BH1, LED-RGB1, LED-V1–V2 (1 nm)",
            mtype: "irradiance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition, Table 12",
            doi: "10.25039/CIE.DS.dhcw57sd",
            columns: led_cols,
        },
        // ── Indoor daylight illuminants ──────────────────────────────────────
        Dataset {
            csv_file: "CIE_illum_ID50.csv",
            subdir: "illuminants",
            json_file: "cie_illum_id50.json",
            title: "CIE Indoor Daylight Illuminant ID50",
            mtype: "irradiance",
            date: "2009-01-01",
            source: "CIE 184:2009 Indoor Daylight Illuminants",
            doi: "10.25039/CIE.DS.r4gcnrzc",
            columns: vec!["ID50".into()],
        },
        Dataset {
            csv_file: "CIE_illum_ID65.csv",
            subdir: "illuminants",
            json_file: "cie_illum_id65.json",
            title: "CIE Indoor Daylight Illuminant ID65",
            mtype: "irradiance",
            date: "2009-01-01",
            source: "CIE 184:2009 Indoor Daylight Illuminants",
            doi: "10.25039/CIE.DS.bd53qdqk",
            columns: vec!["ID65".into()],
        },
        // ── Daylight spectral components ─────────────────────────────────────
        Dataset {
            csv_file: "CIE_illum_Dxx_comp.csv",
            subdir: "illuminants",
            json_file: "cie_illum_daylight_components.json",
            title: "CIE Daylight Spectral Components S0, S1, S2",
            mtype: "irradiance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition",
            doi: "10.25039/CIE.DS.w7zunnny",
            columns: vec!["S0".into(), "S1".into(), "S2".into()],
        },
        // ── Colour rendering test samples ────────────────────────────────────
        Dataset {
            csv_file: "CIE_srf_cri.csv",
            subdir: "color_rendering",
            json_file: "cie_cri_14_test_samples.json",
            title: "CIE Colour Rendering Index — Spectral Radiance Factors of 14 Test Colour Samples",
            mtype: "reflectance",
            date: "1995-01-01",
            source: "CIE 13.3:1995 Method of Measuring and Specifying Colour Rendering Properties of Light Sources, Table A7.1",
            doi: "10.25039/CIE.DS.wuiuu9cz",
            columns: cri_cols,
        },
        Dataset {
            csv_file: "CIE_srf_cfi.csv",
            subdir: "color_rendering",
            json_file: "cie_cfi_99_test_samples_5nm.json",
            title: "CIE Colour Fidelity Index — Spectral Radiance Factors of 99 Test Colour Samples (5 nm)",
            mtype: "reflectance",
            date: "2017-01-01",
            source: "CIE 224:2017 Colour Fidelity Index for Accurate Scientific Use, Table A.1",
            doi: "10.25039/CIE.DS.wi5idbqu",
            columns: cfi_cols.clone(),
        },
        Dataset {
            csv_file: "CIE_srf_cfi_1nm.csv",
            subdir: "color_rendering",
            json_file: "cie_cfi_99_test_samples_1nm.json",
            title: "CIE Colour Fidelity Index — Spectral Radiance Factors of 99 Test Colour Samples (1 nm)",
            mtype: "reflectance",
            date: "2017-01-01",
            source: "CIE 224:2017 Colour Fidelity Index for Accurate Scientific Use, CFI Calculator Toolbox",
            doi: "10.25039/CIE.DS.8svs5rqd",
            columns: cfi_cols,
        },
        Dataset {
            csv_file: "CIE_srf_CQS_5nm.csv",
            subdir: "color_rendering",
            json_file: "cie_cqs_15_test_samples.json",
            title: "CIE Colour Quality Scale — Spectral Radiance Factors of 15 Test Colour Samples (5 nm)",
            mtype: "reflectance",
            date: "2024-01-01",
            source: "CIE 253:2024 Colour Quality Scale, Table A.1",
            doi: "10.25039/CIE.DS.yzfhz3cm",
            columns: cqs_cols,
        },
        Dataset {
            csv_file: "CIE_srf_FCI_5nm.csv",
            subdir: "color_rendering",
            json_file: "cie_four_colour_rygb.json",
            title: "CIE Four-Colour Combination — Spectral Radiance Factors of Red, Yellow, Green, Blue (5 nm)",
            mtype: "reflectance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition",
            doi: "10.25039/CIE.DS.vkss79ef",
            columns: vec!["Red".into(), "Yellow".into(), "Green".into(), "Blue".into()],
        },
        Dataset {
            csv_file: "CIE_srf_PS_5nm.csv",
            subdir: "color_rendering",
            json_file: "cie_japanese_skin_complexion.json",
            title: "CIE Test Colour Sample 15 — Japanese Skin Complexion (5 nm)",
            mtype: "reflectance",
            date: "2018-01-01",
            source: "CIE 015:2018 Colorimetry, 4th Edition",
            doi: "10.25039/CIE.DS.7chm7z5h",
            columns: vec!["beta_15".into()],
        },
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversion helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Count how many data columns (after the wavelength column) are in the first
/// data row of a raw CIE CSV. Used to trim the column-name list to the actual
/// width so no extra names are injected.
fn count_data_cols(csv_content: &str) -> usize {
    for line in csv_content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if t.split(',').next().unwrap_or("").parse::<f64>().is_ok() {
            return t.split(',').count().saturating_sub(1);
        }
    }
    0
}

/// Build the synthetic metadata header block that the CSV reader understands.
fn build_header(ds: &Dataset, n_data_cols: usize) -> String {
    let col_names: Vec<&str> = ds
        .columns
        .iter()
        .map(String::as_str)
        .take(n_data_cols)
        .collect();
    let col_row = format!("wavelength_nm,{}", col_names.join(","));

    format!(
        "Title: {title}\n\
         Measurement_Type: {mtype}\n\
         Date: {date}\n\
         Copyright: \u{00a9} International Commission on Illumination (CIE). \
         Licensed CC BY-SA 4.0. https://cie.co.at/data-tables\n\
         Notes: Source: {source}. DOI: https://doi.org/{doi}\n\
         \n\
         {col_row}\n",
        title = ds.title,
        mtype = ds.mtype,
        date = ds.date,
        source = ds.source,
        doi = ds.doi,
        col_row = col_row,
    )
}

/// Update provenance on every spectrum to record the original CIE CSV URL.
fn set_provenance(file: &mut SpectrumFile, csv_file: &str) {
    let url = format!("https://files.cie.co.at/{csv_file}");
    let fix = |prov: &mut Option<spectral_io::Provenance>| {
        if let Some(p) = prov {
            p.source_file = Some(url.clone());
            p.source_format = Some("CIE CSV".into());
        }
    };
    match file {
        SpectrumFile::Single { spectrum, .. } => fix(&mut spectrum.provenance),
        SpectrumFile::Batch { spectra, .. } => {
            for sp in spectra.iter_mut() {
                fix(&mut sp.provenance);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut input_dir = PathBuf::from("data/cie-raw");
    let mut output_dir = PathBuf::from("data/spectral-io/cie");

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--input" => {
                i += 1;
                if i < args.len() {
                    input_dir = PathBuf::from(&args[i]);
                }
            }
            "--output" => {
                i += 1;
                if i < args.len() {
                    output_dir = PathBuf::from(&args[i]);
                }
            }
            arg => {
                eprintln!("Unknown argument: {arg}");
                process::exit(1);
            }
        }
        i += 1;
    }

    let datasets = datasets();
    let mut ok = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for ds in &datasets {
        let csv_path = input_dir.join(ds.csv_file);
        if !csv_path.exists() {
            eprintln!("  SKIP  {} (file not found)", ds.csv_file);
            skipped += 1;
            continue;
        }

        let raw = match fs::read_to_string(&csv_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  ERROR reading {}: {e}", ds.csv_file);
                failed += 1;
                continue;
            }
        };

        let n_cols = count_data_cols(&raw);
        if n_cols == 0 {
            eprintln!("  ERROR {}: no data rows found", ds.csv_file);
            failed += 1;
            continue;
        }

        let synthetic = format!("{}{}", build_header(ds, n_cols), raw);

        let mut file = match SpectrumFile::from_csv_str(&synthetic) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  ERROR parsing {}: {e}", ds.csv_file);
                failed += 1;
                continue;
            }
        };

        set_provenance(&mut file, ds.csv_file);

        let out_dir = output_dir.join(ds.subdir);
        if let Err(e) = fs::create_dir_all(&out_dir) {
            eprintln!("  ERROR creating {}: {e}", out_dir.display());
            failed += 1;
            continue;
        }

        let json = match serde_json::to_string_pretty(&file) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("  ERROR serialising {}: {e}", ds.json_file);
                failed += 1;
                continue;
            }
        };

        let out_path = out_dir.join(ds.json_file);
        if let Err(e) = fs::write(&out_path, &json) {
            eprintln!("  ERROR writing {}: {e}", out_path.display());
            failed += 1;
            continue;
        }

        let n_spectra = file.spectra().len();
        eprintln!(
            "  OK    {} → {} ({} {})",
            ds.csv_file,
            out_path.display(),
            n_spectra,
            if n_spectra == 1 {
                "spectrum"
            } else {
                "spectra"
            }
        );
        ok += 1;
    }

    eprintln!();
    eprintln!("{ok} converted, {skipped} skipped, {failed} failed");
    if failed > 0 {
        process::exit(1);
    }
}
