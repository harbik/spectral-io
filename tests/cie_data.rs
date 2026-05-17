//! Integration tests for the CIE spectral data files in `data/spectral-io/cie/`.
//!
//! Each test guards with an early return when the generated files are absent so
//! that CI passes even without the pre-built data directory.  To regenerate the
//! files run:
//!
//! ```sh
//! cargo run --example cie_csv_to_json --features csv
//! ```

#![cfg(feature = "csv")]

use spectral_io::SpectrumFile;
use std::path::{Path, PathBuf};

fn cie_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/spectral-io/cie")
}

fn collect_json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_json_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

// ─────────────────────────────────────────────────────────────────────────────
// Validation: every JSON file passes the full schema + cross-field checks
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn all_cie_json_files_are_valid() {
    let dir = cie_dir();
    if !dir.exists() {
        eprintln!(
            "SKIP all_cie_json_files_are_valid: {} not present \
             (run `cargo run --example cie_csv_to_json --features csv` first)",
            dir.display()
        );
        return;
    }

    let files = collect_json_files(&dir);
    assert!(
        !files.is_empty(),
        "no JSON files found in {}",
        dir.display()
    );

    for path in &files {
        let file = SpectrumFile::from_path(path).unwrap_or_else(|e| {
            panic!("{}: {e}", path.display());
        });
        let spectra = file.spectra();
        assert!(!spectra.is_empty(), "{}: no spectra", path.display());
        for sp in &spectra {
            assert!(!sp.id.is_empty(), "{}: empty id", path.display());
            assert!(
                sp.n_points() >= 2,
                "{}: < 2 points in '{}'",
                path.display(),
                sp.id
            );
            let wls = sp.wavelength_axis.wavelengths_nm();
            assert!(
                wls.windows(2).all(|w| w[0] < w[1]),
                "{}: non-monotonic wavelengths in '{}'",
                path.display(),
                sp.id
            );
        }
    }

    eprintln!("validated {} CIE JSON files", files.len());
}

// ─────────────────────────────────────────────────────────────────────────────
// Round-trip: JSON → csv_write → from_csv_str, values and IDs survive
// ─────────────────────────────────────────────────────────────────────────────

fn round_trip(path: &Path) {
    let file1 = SpectrumFile::from_path(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let csv = file1.to_csv();

    let file2 = SpectrumFile::from_csv_str(&csv)
        .unwrap_or_else(|e| panic!("{}: round-trip CSV parse failed: {e}", path.display()));

    let s1 = file1.spectra();
    let s2 = file2.spectra();

    assert_eq!(
        s1.len(),
        s2.len(),
        "{}: spectrum count changed after round-trip",
        path.display()
    );

    for (a, b) in s1.iter().zip(s2.iter()) {
        assert_eq!(a.id, b.id, "{}: id changed for '{}'", path.display(), a.id);
        assert_eq!(
            a.n_points(),
            b.n_points(),
            "{}: point count changed for '{}'",
            path.display(),
            a.id
        );
        for (i, ((_, va), (_, vb))) in a.points().iter().zip(b.points().iter()).enumerate() {
            assert!(
                (va - vb).abs() < 1e-9,
                "{}: value mismatch at point {i} for '{}': {va} vs {vb}",
                path.display(),
                a.id
            );
        }
    }
}

#[test]
fn round_trip_single_spectrum_illuminants() {
    let dir = cie_dir().join("illuminants");
    if !dir.exists() {
        eprintln!("SKIP round_trip_single_spectrum_illuminants: data not present");
        return;
    }
    for name in &[
        "cie_std_illum_a.json",
        "cie_std_illum_d50.json",
        "cie_std_illum_d65.json",
        "cie_std_illum_c.json",
        "cie_std_illum_d55.json",
        "cie_std_illum_d75.json",
        "cie_illum_id50.json",
        "cie_illum_id65.json",
    ] {
        let p = dir.join(name);
        if p.exists() {
            round_trip(&p);
        }
    }
}

#[test]
fn round_trip_batch_illuminants() {
    let dir = cie_dir().join("illuminants");
    if !dir.exists() {
        eprintln!("SKIP round_trip_batch_illuminants: data not present");
        return;
    }
    for name in &[
        "cie_illum_hp_lamps.json",
        "cie_illum_fl_lamps_5nm.json",
        "cie_illum_led_lamps_5nm.json",
        "cie_illum_daylight_components.json",
    ] {
        let p = dir.join(name);
        if p.exists() {
            round_trip(&p);
        }
    }
}

#[test]
fn round_trip_color_rendering_samples() {
    let dir = cie_dir().join("color_rendering");
    if !dir.exists() {
        eprintln!("SKIP round_trip_color_rendering_samples: data not present");
        return;
    }
    for name in &[
        "cie_cri_14_test_samples.json",
        "cie_cqs_15_test_samples.json",
        "cie_four_colour_rygb.json",
        "cie_japanese_skin_complexion.json",
    ] {
        let p = dir.join(name);
        if p.exists() {
            round_trip(&p);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Content spot-checks on known values
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn d65_spot_check() {
    let path = cie_dir().join("illuminants/cie_std_illum_d65.json");
    if !path.exists() {
        eprintln!("SKIP d65_spot_check: data not present");
        return;
    }
    let file = SpectrumFile::from_path(&path).unwrap();
    let sp = file.spectra()[0];
    assert_eq!(sp.id, "D65");
    // D65 is defined over 300–830 nm at 1 nm: 531 points
    assert_eq!(sp.n_points(), 531);
    let (start, end) = sp.wavelength_range_nm().unwrap();
    assert!((start - 300.0).abs() < 0.01);
    assert!((end - 830.0).abs() < 0.01);
    // The Y tristimulus value of D65 normalised to Y=100 is 100.000 (by definition);
    // as a spot check, verify the value at 560 nm (roughly the peak of V(λ)) is
    // substantial (> 80) and at 300 nm is near zero (< 10).
    let pts = sp.points();
    let at_300 = pts[0].1;
    assert!(at_300 < 10.0, "D65 at 300 nm should be small, got {at_300}");
    let at_560 = pts[260].1; // 300 + 260 = 560
    assert!(
        at_560 > 80.0,
        "D65 at 560 nm should be substantial, got {at_560}"
    );
}

#[test]
fn cri_14_spot_check() {
    let path = cie_dir().join("color_rendering/cie_cri_14_test_samples.json");
    if !path.exists() {
        eprintln!("SKIP cri_14_spot_check: data not present");
        return;
    }
    let file = SpectrumFile::from_path(&path).unwrap();
    let spectra = file.spectra();
    assert_eq!(spectra.len(), 14, "expected 14 CRI test colour samples");
    assert_eq!(spectra[0].id, "TCS01");
    assert_eq!(spectra[13].id, "TCS14");
    // All values must be reflectances in [0, 1]
    for sp in &spectra {
        for (_, v) in sp.points() {
            assert!(
                (0.0..=1.0).contains(&v),
                "reflectance out of range [0,1] for '{}': {v}",
                sp.id
            );
        }
    }
}

#[test]
fn fl_batch_spot_check() {
    let path = cie_dir().join("illuminants/cie_illum_fl_lamps_5nm.json");
    if !path.exists() {
        eprintln!("SKIP fl_batch_spot_check: data not present");
        return;
    }
    let file = SpectrumFile::from_path(&path).unwrap();
    let spectra = file.spectra();
    assert_eq!(spectra.len(), 27, "expected 27 FL illuminants");
    assert_eq!(spectra[0].id, "FL1");
    assert_eq!(spectra[11].id, "FL12");
    assert_eq!(spectra[12].id, "FL3.1");
    assert_eq!(spectra[26].id, "FL3.15");
}

#[test]
fn led_batch_spot_check() {
    let path = cie_dir().join("illuminants/cie_illum_led_lamps_5nm.json");
    if !path.exists() {
        eprintln!("SKIP led_batch_spot_check: data not present");
        return;
    }
    let file = SpectrumFile::from_path(&path).unwrap();
    let spectra = file.spectra();
    assert_eq!(spectra[0].id, "LED-B1");
    assert_eq!(spectra[5].id, "LED-BH1");
    assert_eq!(spectra[6].id, "LED-RGB1");
}
