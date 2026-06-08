use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyIOError, PyRuntimeError};
use pyo3::prelude::*;

use grammers_mtsender::{InvocationError, RpcError};
use grammers_tl_types as tl;

create_exception!(telethon_mtsender, DroppedError, PyException);
create_exception!(telethon_mtsender, DeserializeError, PyException);
create_exception!(telethon_mtsender, TransportError, PyException);

#[pyclass(name = "RpcError", module = "telethon_mtsender", extends = PyException, subclass, dict)]
pub struct PyRpcError {
    inner: RpcError,
}

impl PyRpcError {
    pub fn new_err(code: i32, name: String, value: Option<u32>, caused_by: Option<u32>) -> PyErr {
        PyErr::new::<PyRpcError, _>((code, name, value, caused_by))
    }

    pub fn from(err: RpcError) -> PyErr {
        let err = PyRpcError { inner: err };
        Python::attach(|py| match Py::new(py, err) {
            Ok(x) => PyErr::from_value(x.into_bound(py).into_any()),
            Err(e) => e,
        })
    }
}

#[pymethods]
impl PyRpcError {
    #[new]
    #[pyo3(signature = (code, name, value=None, caused_by=None))]
    fn new(code: i32, name: String, value: Option<u32>, caused_by: Option<u32>) -> Self {
        Self {
            inner: RpcError {
                code,
                name,
                value,
                caused_by,
            },
        }
    }

    #[getter]
    fn code(&self) -> i32 {
        self.inner.code
    }

    #[setter(code)]
    fn set_code(&mut self, code: i32) {
        self.inner.code = code;
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    #[setter(name)]
    fn set_name(&mut self, name: String) {
        self.inner.name = name;
    }

    #[getter]
    fn value(&self) -> Option<u32> {
        self.inner.value
    }

    #[setter(value)]
    fn set_value(&mut self, value: Option<u32>) {
        self.inner.value = value;
    }

    #[getter]
    fn caused_by(&self) -> Option<u32> {
        self.inner.caused_by
    }

    #[setter(caused_by)]
    fn set_caused_by(&mut self, constructor_id: Option<u32>) {
        self.inner.caused_by = constructor_id
    }

    #[pyo3(name = "is_rpc")]
    fn is(&self, rpc_error: &str) -> bool {
        self.inner.is(rpc_error)
    }

    fn __str__(&self) -> String {
        format!("{}", self.inner)
    }

    fn __repr__(&self) -> PyResult<String> {
        let value = match self.value() {
            Some(x) => x.to_string(),
            None => "None".to_string(),
        };
        let caused_by = match self.caused_by() {
            Some(x) => tl::name_for_id(x),
            None => "None",
        };
        Ok(format!(
            "RpcError(\n\tcode={},\n\tname={},\n\tvalue={},\n\tcaused_by={},\n)",
            self.code(),
            self.name(),
            value,
            caused_by,
        ))
    }
}

pub(crate) fn convert_invocation_error(err: InvocationError) -> PyErr {
    match err {
        InvocationError::Rpc(e) => PyRpcError::from(e),
        InvocationError::Io(e) => PyIOError::new_err(e.to_string()),
        InvocationError::Deserialize(e) => DeserializeError::new_err(e.to_string()),
        InvocationError::Transport(e) => TransportError::new_err(e.to_string()),
        InvocationError::Dropped => DroppedError::new_err(
            "The sender has not start, and the sent data packet was dropped.".to_string(),
        ),
        InvocationError::InvalidDc => PyRuntimeError::new_err(String::from(
            "The session returns no infomation of the provided dc_id",
        )),
        InvocationError::Authentication(e) => {
            PyRuntimeError::new_err(format!("Authentication: {}", e))
        }
    }
}
