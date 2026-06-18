use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;

use grammers_crypto::two_factor_auth;

#[pyfunction]
#[pyo3(signature = (
    *,
    salt1,
    salt2,
    p,
    g,
    g_b,
    a,
    password,
))]
pub fn calculate_2fa(
    py: Python<'_>,
    salt1: PyBuffer<u8>,
    salt2: PyBuffer<u8>,
    p: PyBuffer<u8>,
    g: i32,
    g_b: PyBuffer<u8>,
    a: PyBuffer<u8>,
    password: PyBuffer<u8>,
) -> PyResult<([u8; 32], [u8; 256])> {
    let salt1 = salt1.to_vec(py)?;
    let salt2 = salt2.to_vec(py)?;
    let p = p.to_vec(py)?;
    let g_b = g_b.to_vec(py)?;
    let a = a.to_vec(py)?;
    let password = password.to_vec(py)?;

    Ok(two_factor_auth::calculate_2fa(
        &salt1, &salt2, &p, &g, g_b, a, &password,
    ))
}

#[pyfunction]
#[pyo3(signature = (p, g))]
pub fn check_p_and_g(py: Python<'_>, p: PyBuffer<u8>, g: i32) -> PyResult<bool> {
    let p = p.to_vec(py)?;

    Ok(two_factor_auth::check_p_and_g(&p, &g))
}
