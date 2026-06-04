use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyIOError, PyRuntimeError};
use pyo3::prelude::*;

use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;

create_exception!(telethon_mtsender, DroppedError, PyException);
create_exception!(telethon_mtsender, DeserializeError, PyException);
create_exception!(telethon_mtsender, TransportError, PyException);

#[pyclass(name = "RpcError", module = "telethon_mtsender", extends = PyException, subclass)]
pub struct PyRpcError {
    #[pyo3(get)]
    pub code: i32,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub value: Option<u32>,
    #[pyo3(get)]
    pub caused_by: Option<u32>,
    #[pyo3(get)]
    pub message: Option<String>,
}

impl PyRpcError {
    pub fn new_err(
        code: i32,
        name: String,
        value: Option<u32>,
        caused_by: Option<u32>,
        message: Option<String>,
    ) -> PyErr {
        PyErr::new::<PyRpcError, _>((code, name, value, caused_by, message))
    }
}

#[pymethods]
impl PyRpcError {
    #[new]
    #[pyo3(signature = (code, name, value, caused_by=None, message=None))]
    fn new(
        code: i32,
        name: String,
        value: Option<u32>,
        caused_by: Option<u32>,
        message: Option<String>,
    ) -> Self {
        Self {
            code,
            name,
            value,
            caused_by,
            message,
        }
    }

    fn __str__(&self) -> PyResult<String> {
        let message = match &self.message {
            Some(x) => x,
            None => &"".to_string(),
        };
        let caused_by = match self.caused_by {
            None => "".to_string(),
            Some(x) => format!("caused by {}", tl::name_for_id(x)),
        };
        let value = match self.value {
            None => "".to_string(),
            Some(x) => format!("with value: {}", x),
        };
        let more = vec![self.name.clone(), caused_by, value]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<String>>()
            .join(", ");
        let more = if more.is_empty() {
            "".to_string()
        } else {
            format!(" {}", more)
        };
        Ok(format!("{} ({}{})", message, self.code, more))
    }

    fn __repr__(&self) -> PyResult<String> {
        let value = match self.value {
            Some(x) => x.to_string(),
            None => "None".to_string(),
        };
        let caused_by = match self.caused_by {
            Some(x) => tl::name_for_id(x),
            None => "None",
        };
        let message = match &self.message {
            Some(x) => x,
            None => &"".to_string(),
        };
        Ok(format!(
            "RpcError(\n  code={},\n  name={},\n  value={},\n  caused_by={},\n  message={},\n)",
            self.code, self.name, value, caused_by, message,
        ))
    }
}

pub(crate) fn convert_invocation_error(err: InvocationError) -> PyErr {
    match err {
        InvocationError::Rpc(e) => PyRpcError::new_err(e.code, e.name, e.value, e.caused_by, None),
        InvocationError::Io(e) => PyIOError::new_err(e.to_string()),
        InvocationError::Deserialize(e) => DeserializeError::new_err(e.to_string()),
        InvocationError::Transport(e) => TransportError::new_err(e.to_string()),
        InvocationError::Dropped => DroppedError::new_err(
            "Client runner not start, and the sent data packet dropped. Pleace re-create Client() to restart.".to_string(),
        ),
        InvocationError::InvalidDc => PyRuntimeError::new_err(
            String::from(
                "Session returns no infomation of the provided dc_id"
            )
        ),
        InvocationError::Authentication(e) => {
            PyRuntimeError::new_err(format!("Authentication: {}", e))
        }
    }
}
