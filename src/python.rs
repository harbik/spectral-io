// PyO3 0.22 macros generate `PyErr::from(pyerr)` wrapper code whose spans
// point back to the user return-type annotation, triggering this lint.
#![allow(clippy::useless_conversion)]

use numpy::ndarray::{Array1, Array2};
use numpy::IntoPyArray;
use pyo3::prelude::*;

use crate::{ResampleMethod, SpectrumFile, SpectrumFileError, WavelengthAxis, WavelengthRange};

impl From<SpectrumFileError> for PyErr {
    fn from(e: SpectrumFileError) -> Self {
        match e {
            SpectrumFileError::Io(io_err) => {
                pyo3::exceptions::PyIOError::new_err(io_err.to_string())
            }
            other => pyo3::exceptions::PyValueError::new_err(other.to_string()),
        }
    }
}

/// A loaded spectrum file, giving access to the spectra it contains.
#[pyclass(name = "SpectrumFile")]
struct PySpectrumFile {
    inner: SpectrumFile,
}

#[allow(clippy::useless_conversion)]
#[pymethods]
impl PySpectrumFile {
    /// List of spectrum IDs in file order.
    #[getter]
    fn ids(&self) -> Vec<String> {
        self.inner.spectra().iter().map(|s| s.id.clone()).collect()
    }

    /// Resample all spectra onto an equidistant grid and return NumPy arrays.
    ///
    /// Returns ``(wavelengths, data)`` where:
    ///
    /// * ``wavelengths`` — 1-D float64 array of shape ``(n,)``
    /// * ``data`` — 1-D float64 array of shape ``(n,)`` for a single-spectrum
    ///   file, or a 2-D float64 array of shape ``(n, m)`` for a batch file
    ///   where each column is one spectrum in the same order as ``ids``.
    ///
    /// ``method`` must be one of ``"linear"`` (default), ``"boxcar_average"``,
    /// or ``"gaussian"``.
    #[pyo3(signature = (start, end, interval, method = "linear"))]
    fn to_numpy(
        &self,
        py: Python<'_>,
        start: f64,
        end: f64,
        interval: f64,
        method: &str,
    ) -> PyResult<(PyObject, PyObject)> {
        let resample_method = match method {
            "linear" => ResampleMethod::Linear,
            "boxcar_average" => ResampleMethod::BoxcarAverage,
            "gaussian" => ResampleMethod::Gaussian,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown method '{other}'; choose 'linear', 'boxcar_average', or 'gaussian'"
                )))
            }
        };

        let target = WavelengthAxis {
            range_nm: Some(WavelengthRange {
                start,
                end,
                interval,
            }),
            values_nm: None,
        };

        let wavelengths = target.wavelengths_nm();
        let wl_obj: PyObject = Array1::from(wavelengths.clone())
            .into_pyarray_bound(py)
            .unbind()
            .into_any();

        let spectra = self.inner.spectra();
        let n = wavelengths.len();

        let data_obj: PyObject = if spectra.len() == 1 {
            let values = spectra[0]
                .resample(&target, resample_method)
                .spectral_data
                .values;
            Array1::from(values)
                .into_pyarray_bound(py)
                .unbind()
                .into_any()
        } else {
            let m = spectra.len();
            let mut matrix = Array2::<f64>::zeros((n, m));
            for (j, sp) in spectra.iter().enumerate() {
                let values = sp.resample(&target, resample_method).spectral_data.values;
                for (i, &v) in values.iter().enumerate().take(n) {
                    matrix[[i, j]] = v;
                }
            }
            matrix.into_pyarray_bound(py).unbind().into_any()
        };

        Ok((wl_obj, data_obj))
    }
}

/// Load and validate a spectral JSON file from a file path.
#[allow(clippy::useless_conversion)]
#[pyfunction]
fn load(path: &str) -> PyResult<PySpectrumFile> {
    Ok(PySpectrumFile {
        inner: SpectrumFile::from_path(path)?,
    })
}

/// Load and validate a spectral file from a JSON string.
#[allow(clippy::useless_conversion)]
#[pyfunction]
fn load_json(json: &str) -> PyResult<PySpectrumFile> {
    Ok(PySpectrumFile {
        inner: SpectrumFile::from_json_str(json)?,
    })
}

#[pymodule]
pub fn spectral_io(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySpectrumFile>()?;
    m.add_function(wrap_pyfunction!(load, m)?)?;
    m.add_function(wrap_pyfunction!(load_json, m)?)?;
    Ok(())
}
