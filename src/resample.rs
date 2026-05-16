//! Resampling of [`SpectrumRecord`] onto a new wavelength axis.

use super::*;

/// Method used when resampling a spectrum to a new wavelength axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResampleMethod {
    /// Linear interpolation between adjacent input samples.
    ///
    /// Each output value is computed by linearly interpolating between the two
    /// nearest input wavelengths. Output wavelengths outside the input range
    /// are clamped to the nearest endpoint (no extrapolation).
    ///
    /// Works for both upsampling and downsampling.
    Linear,

    /// Boxcar (rectangular window) averaging.
    ///
    /// For each output wavelength `λ`, all input samples within the half-step
    /// window `[λ − step/2, λ + step/2]` are averaged, where `step` is the
    /// mean spacing of the target axis.  Falls back to linear interpolation
    /// for any output wavelength whose window contains no input samples.
    ///
    /// Most appropriate when downsampling to a coarser grid (e.g. 1 nm → 10 nm).
    ///
    /// **Assumes a regular (uniformly-spaced) target grid.** The window
    /// half-width is derived from the mean spacing of the entire target axis;
    /// for irregular target grids the bins may overlap or leave gaps.  Use a
    /// `WavelengthAxis` with `range_nm` (start / end / interval) to guarantee
    /// a regular grid.
    BoxcarAverage,

    /// Gaussian kernel resampling.
    ///
    /// Each output value is a weighted average of the input samples, where the
    /// weight of input sample at `w` is `exp(−½ ((w − λ) / σ)²)`.  Samples
    /// further than 3σ from the output wavelength are excluded.  Weights are
    /// normalised by their sum, so output values at the edges of the input
    /// range are not artificially attenuated.
    ///
    /// The kernel FWHM is resolved in this order:
    ///
    /// 1. `metadata.measurement_conditions.spectral_resolution_nm` — the
    ///    instrument's optical resolution, physically the most meaningful
    ///    choice.
    /// 2. Mean step size of the target axis — used as a fallback when no
    ///    resolution is recorded, matching the kernel width to the output
    ///    sampling interval.
    ///
    /// σ is derived from the FWHM as `FWHM / (2√(2 ln 2)) ≈ FWHM / 2.355`.
    ///
    /// If the FWHM is much smaller than the input sampling interval the kernel
    /// degenerates toward nearest-neighbour interpolation; if no input samples
    /// fall within the 3σ window the method falls back to linear interpolation.
    Gaussian,
}

impl SpectrumRecord {
    /// Resample this spectrum onto `target`, returning a new [`SpectrumRecord`].
    ///
    /// The source spectrum's metadata, colour-science block, and provenance are
    /// preserved; a [`ProcessingStep`] describing the operation is appended to
    /// the provenance trail.
    ///
    /// # Preconditions
    ///
    /// The source wavelength axis must be sorted in ascending order.
    /// `WavelengthAxis` values produced by this library always satisfy this
    /// requirement; an unsorted axis will produce silently incorrect output.
    ///
    /// # Uncertainty
    ///
    /// Any `uncertainty` values on the source spectrum are **not** carried
    /// forward — the returned `SpectralData` always has `uncertainty: None`.
    /// Correct propagation of uncertainty through interpolation and averaging
    /// requires knowledge of the correlation structure of the input errors and
    /// is left to the caller.
    pub fn resample(&self, target: &WavelengthAxis, method: ResampleMethod) -> Self {
        let input_wls = self.wavelength_axis.wavelengths_nm();
        let input_vals = &self.spectral_data.values;
        let target_wls = target.wavelengths_nm();

        let (values, fwhm_used): (Vec<f64>, Option<f64>) = match method {
            ResampleMethod::Linear => (
                target_wls
                    .iter()
                    .map(|&wl| linear_interp(&input_wls, input_vals, wl))
                    .collect(),
                None,
            ),
            ResampleMethod::BoxcarAverage => {
                let half_step = mean_half_step(&target_wls);
                (
                    target_wls
                        .iter()
                        .map(|&wl| boxcar_avg(&input_wls, input_vals, wl, half_step))
                        .collect(),
                    None,
                )
            }
            ResampleMethod::Gaussian => {
                let fwhm = self
                    .metadata
                    .measurement_conditions
                    .as_ref()
                    .and_then(|mc| mc.spectral_resolution_nm)
                    .unwrap_or_else(|| mean_half_step(&target_wls) * 2.0);
                let sigma = fwhm / (8.0_f64 * 2.0_f64.ln()).sqrt();
                (
                    target_wls
                        .iter()
                        .map(|&wl| gaussian_avg(&input_wls, input_vals, wl, sigma))
                        .collect(),
                    Some(fwhm),
                )
            }
        };

        let step = provenance_step(&target_wls, method, fwhm_used);
        let provenance = Some(match self.provenance.clone() {
            Some(mut p) => {
                let steps = p.processing_steps.get_or_insert_with(Vec::new);
                steps.push(step);
                p
            }
            None => Provenance {
                software: None,
                software_version: None,
                source_file: None,
                source_format: None,
                processing_steps: Some(vec![step]),
                notes: None,
            },
        });

        Self {
            wavelength_axis: target.clone(),
            spectral_data: SpectralData {
                values,
                uncertainty: None,
                scale: self.spectral_data.scale.clone(),
            },
            provenance,
            ..self.clone()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn linear_interp(wls: &[f64], vals: &[f64], target: f64) -> f64 {
    debug_assert!(!wls.is_empty() && wls.len() == vals.len());
    let i = wls.partition_point(|&w| w < target);
    match i {
        0 => vals[0],
        i if i == wls.len() => vals[wls.len() - 1],
        i => {
            let t = (target - wls[i - 1]) / (wls[i] - wls[i - 1]);
            vals[i - 1] + t * (vals[i] - vals[i - 1])
        }
    }
}

fn boxcar_avg(wls: &[f64], vals: &[f64], target: f64, half_step: f64) -> f64 {
    let lo = target - half_step;
    let hi = target + half_step;
    let mut sum = 0.0_f64;
    let mut count = 0usize;
    for (&w, &v) in wls.iter().zip(vals.iter()) {
        if w >= lo && w <= hi {
            sum += v;
            count += 1;
        }
    }
    if count > 0 {
        sum / count as f64
    } else {
        linear_interp(wls, vals, target)
    }
}

fn gaussian_avg(wls: &[f64], vals: &[f64], target: f64, sigma: f64) -> f64 {
    let cutoff = 3.0 * sigma;
    let mut weight_sum = 0.0_f64;
    let mut value_sum = 0.0_f64;
    for (&w, &v) in wls.iter().zip(vals.iter()) {
        let d = w - target;
        if d.abs() <= cutoff {
            let weight = (-0.5 * (d / sigma) * (d / sigma)).exp();
            value_sum += weight * v;
            weight_sum += weight;
        }
    }
    if weight_sum > 0.0 {
        value_sum / weight_sum
    } else {
        linear_interp(wls, vals, target)
    }
}

// Mean half-step: (last − first) / (2 × (n − 1)).
// For a regular grid this is exactly interval / 2.
fn mean_half_step(wls: &[f64]) -> f64 {
    if wls.len() < 2 {
        return 0.0;
    }
    (wls.last().unwrap() - wls[0]) / (2.0 * (wls.len() - 1) as f64)
}

fn provenance_step(
    target_wls: &[f64],
    method: ResampleMethod,
    fwhm_nm: Option<f64>,
) -> ProcessingStep {
    let method_name = match method {
        ResampleMethod::Linear => "linear interpolation",
        ResampleMethod::BoxcarAverage => "boxcar average",
        ResampleMethod::Gaussian => "Gaussian",
    };
    let n = target_wls.len();
    let desc = if n >= 2 {
        let base = format!(
            "{method_name} to {n} points, {:.4}–{:.4} nm",
            target_wls[0],
            target_wls[n - 1]
        );
        match fwhm_nm {
            Some(fwhm) => format!("{base}, FWHM {fwhm:.4} nm"),
            None => base,
        }
    } else {
        format!("{method_name} to {n} point(s)")
    };
    ProcessingStep {
        step: "resample".into(),
        description: desc,
        parameters: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal SpectrumRecord for testing.
    fn make_record(wls: &[f64], vals: &[f64]) -> SpectrumRecord {
        SpectrumRecord {
            id: "test".into(),
            metadata: SpectrumMetadata {
                measurement_type: MeasurementType::Reflectance,
                date: "2026-05-16".into(),
                title: None,
                description: None,
                sample_id: None,
                time: None,
                operator: None,
                instrument: None,
                measurement_conditions: None,
                surface: None,
                sample_backing: None,
                tags: None,
                copyright: None,
                custom: None,
            },
            wavelength_axis: WavelengthAxis {
                values_nm: Some(wls.to_vec()),
                range_nm: None,
            },
            spectral_data: SpectralData {
                values: vals.to_vec(),
                uncertainty: None,
                scale: None,
            },
            color_science: None,
            provenance: None,
        }
    }

    fn regular_target(start: f64, end: f64, step: f64) -> WavelengthAxis {
        WavelengthAxis {
            values_nm: None,
            range_nm: Some(WavelengthRange {
                start,
                end,
                interval: step,
            }),
        }
    }

    // ── Linear ───────────────────────────────────────────────────────────────

    #[test]
    fn linear_identity() {
        let wls = [380.0, 390.0, 400.0];
        let vals = [0.1, 0.2, 0.3];
        let sp = make_record(&wls, &vals);
        let target = regular_target(380.0, 400.0, 10.0);
        let out = sp.resample(&target, ResampleMethod::Linear);
        assert_eq!(out.spectral_data.values, vals);
    }

    #[test]
    fn linear_upsample_midpoint() {
        // Linear function: val = (wl - 380) / 100 — interpolated midpoints should be exact.
        let wls: Vec<f64> = (0..=4).map(|i| 380.0 + i as f64 * 10.0).collect();
        let vals: Vec<f64> = wls.iter().map(|&w| (w - 380.0) / 100.0).collect();
        let sp = make_record(&wls, &vals);
        let target = regular_target(380.0, 420.0, 5.0);
        let out = sp.resample(&target, ResampleMethod::Linear);
        for (wl, &v) in target
            .wavelengths_nm()
            .iter()
            .zip(out.spectral_data.values.iter())
        {
            let expected = (wl - 380.0) / 100.0;
            assert!(
                (v - expected).abs() < 1e-12,
                "at {wl}: got {v}, expected {expected}"
            );
        }
    }

    #[test]
    fn linear_clamps_below_range() {
        let sp = make_record(&[390.0, 400.0], &[0.5, 0.6]);
        let target = regular_target(380.0, 400.0, 10.0);
        let out = sp.resample(&target, ResampleMethod::Linear);
        // 380 nm is below the input range — should clamp to vals[0] = 0.5
        assert_eq!(out.spectral_data.values[0], 0.5);
    }

    #[test]
    fn linear_clamps_above_range() {
        let sp = make_record(&[380.0, 390.0], &[0.5, 0.6]);
        let target = regular_target(380.0, 400.0, 10.0);
        let out = sp.resample(&target, ResampleMethod::Linear);
        // 400 nm is above the input range — should clamp to vals.last() = 0.6
        assert_eq!(*out.spectral_data.values.last().unwrap(), 0.6);
    }

    // ── BoxcarAverage ─────────────────────────────────────────────────────────

    #[test]
    fn boxcar_downsample_averages_bins() {
        // Input: 380–400 nm at 2 nm, constant value 0.5.  Downsample to 10 nm.
        // Each output bin [lo, hi] contains 6 input points (e.g. 375–385 contains
        // 376,378,380,382,384; but with lo=375 and first point at 380 the bin
        // [375,385] contains 380,382,384 — let's just test the average equals 0.5.
        let wls: Vec<f64> = (0..=10).map(|i| 380.0 + i as f64 * 2.0).collect();
        let vals = vec![0.5_f64; wls.len()];
        let sp = make_record(&wls, &vals);
        let target = regular_target(380.0, 400.0, 10.0);
        let out = sp.resample(&target, ResampleMethod::BoxcarAverage);
        for &v in &out.spectral_data.values {
            assert!((v - 0.5).abs() < 1e-12);
        }
    }

    #[test]
    fn boxcar_downsample_correct_average() {
        // Input: 380, 382, 384, 386, 388, 390 at values 1,2,3,4,5,6.
        // Target: 380–390 at 10 nm → two bins [375,385] and [385,395].
        // Bin at 380: contains 380,382,384 → avg = 2.0
        // Bin at 390: contains 386,388,390 → avg = 5.0
        let wls = vec![380.0, 382.0, 384.0, 386.0, 388.0, 390.0];
        let vals = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let sp = make_record(&wls, &vals);
        let target = regular_target(380.0, 390.0, 10.0);
        let out = sp.resample(&target, ResampleMethod::BoxcarAverage);
        assert!((out.spectral_data.values[0] - 2.0).abs() < 1e-12);
        assert!((out.spectral_data.values[1] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn boxcar_empty_bin_falls_back_to_linear() {
        // Upsample: input at 380, 400 — output at 380, 390, 400 (step=10, half=5).
        // Bin at 390 is [385, 395] — no input point falls there → linear fallback.
        let sp = make_record(&[380.0, 400.0], &[0.0, 1.0]);
        let target = regular_target(380.0, 400.0, 10.0);
        let out = sp.resample(&target, ResampleMethod::BoxcarAverage);
        // Linear fallback at 390: 0.5
        assert!((out.spectral_data.values[1] - 0.5).abs() < 1e-12);
    }

    // ── Provenance ────────────────────────────────────────────────────────────

    #[test]
    fn resample_appends_processing_step() {
        let sp = make_record(&[380.0, 390.0, 400.0], &[0.1, 0.2, 0.3]);
        let target = regular_target(380.0, 400.0, 5.0);
        let out = sp.resample(&target, ResampleMethod::Linear);
        let steps = out.provenance.unwrap().processing_steps.unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].step, "resample");
        assert!(steps[0].description.contains("linear interpolation"));
    }

    #[test]
    fn resample_appends_to_existing_provenance() {
        let mut sp = make_record(&[380.0, 390.0, 400.0], &[0.1, 0.2, 0.3]);
        sp.provenance = Some(Provenance {
            software: Some("TestSuite".into()),
            software_version: None,
            source_file: None,
            source_format: None,
            processing_steps: Some(vec![ProcessingStep {
                step: "trim".into(),
                description: "trimmed to 380–400 nm".into(),
                parameters: None,
            }]),
            notes: None,
        });
        let target = regular_target(380.0, 400.0, 5.0);
        let out = sp.resample(&target, ResampleMethod::BoxcarAverage);
        let prov = out.provenance.unwrap();
        assert_eq!(prov.software.as_deref(), Some("TestSuite"));
        let steps = prov.processing_steps.unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[1].step, "resample");
        assert!(steps[1].description.contains("boxcar average"));
    }

    #[test]
    fn resample_preserves_metadata_and_scale() {
        let mut sp = make_record(&[380.0, 390.0, 400.0], &[0.1, 0.2, 0.3]);
        sp.metadata.title = Some("My Sample".into());
        sp.spectral_data.scale = Some("fractional".into());
        let target = regular_target(380.0, 400.0, 5.0);
        let out = sp.resample(&target, ResampleMethod::Linear);
        assert_eq!(out.metadata.title.as_deref(), Some("My Sample"));
        assert_eq!(out.spectral_data.scale.as_deref(), Some("fractional"));
        assert_eq!(out.id, "test");
    }

    // ── Gaussian ──────────────────────────────────────────────────────────────

    #[test]
    fn gaussian_constant_spectrum() {
        // A constant input must remain constant regardless of kernel width.
        let wls: Vec<f64> = (0..=20).map(|i| 380.0 + i as f64).collect();
        let vals = vec![0.5_f64; wls.len()];
        let sp = make_record(&wls, &vals);
        let target = regular_target(380.0, 400.0, 5.0);
        let out = sp.resample(&target, ResampleMethod::Gaussian);
        for &v in &out.spectral_data.values {
            assert!((v - 0.5).abs() < 1e-12, "got {v}");
        }
    }

    #[test]
    fn gaussian_uses_spectral_resolution_nm() {
        // spectral_resolution_nm = 5.0 → FWHM 5.0 nm appears in provenance.
        let mut sp = make_record(&[380.0, 390.0, 400.0], &[0.1, 0.2, 0.3]);
        sp.metadata.measurement_conditions = Some(MeasurementConditions {
            spectral_resolution_nm: Some(5.0),
            integration_time_ms: None,
            averaging: None,
            temperature_celsius: None,
            geometry: None,
            specular_component: None,
            measurement_aperture_mm: None,
            measurement_filter: None,
        });
        let target = regular_target(380.0, 400.0, 5.0);
        let out = sp.resample(&target, ResampleMethod::Gaussian);
        let desc = out
            .provenance
            .unwrap()
            .processing_steps
            .unwrap()
            .remove(0)
            .description;
        assert!(desc.contains("Gaussian"), "{desc}");
        assert!(desc.contains("5.0000"), "{desc}");
    }

    #[test]
    fn gaussian_fallback_fwhm_equals_target_step() {
        // No spectral_resolution_nm → FWHM falls back to mean target step (5.0 nm).
        let sp = make_record(&[380.0, 390.0, 400.0], &[0.1, 0.2, 0.3]);
        let target = regular_target(380.0, 400.0, 5.0);
        let out = sp.resample(&target, ResampleMethod::Gaussian);
        let desc = out
            .provenance
            .unwrap()
            .processing_steps
            .unwrap()
            .remove(0)
            .description;
        assert!(desc.contains("5.0000"), "{desc}");
    }

    #[test]
    fn gaussian_normalized_at_boundary() {
        // Constant input: output at the very edges of the range must equal the
        // constant (normalization by weight sum, not by a fixed count).
        let wls: Vec<f64> = (0..=20).map(|i| 380.0 + i as f64).collect();
        let vals = vec![0.7_f64; wls.len()];
        let sp = make_record(&wls, &vals);
        // Target extends slightly beyond input range — edge bins have fewer
        // contributing input samples but weights still sum to 1.
        let target = regular_target(378.0, 402.0, 2.0);
        let out = sp.resample(&target, ResampleMethod::Gaussian);
        for &v in &out.spectral_data.values {
            assert!((v - 0.7).abs() < 1e-12, "got {v}");
        }
    }
}
