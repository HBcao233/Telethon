use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::get_runtime;

use grammers_mtproto::{mtp, transport};
use grammers_mtsender::{InvocationError, Sender, ServerAddr, connect, connect_with_auth};
use grammers_tl_types as tl;

use crate::errors::convert_invocation_error;

struct TLObject(pub Vec<u8>);
impl tl::Deserializable for TLObject {
    fn deserialize<'a, 'b>(buf: &'a mut tl::Cursor<'b>) -> tl::deserialize::Result<Self> {
        let mut buffer = Vec::new();
        let _ = buf.read_to_end(&mut buffer);
        Ok(Self(buffer))
    }
}

struct TLRequest(pub Vec<u8>);
impl tl::Serializable for TLRequest {
    fn serialize(&self, buf: &mut impl Extend<u8>) {
        buf.extend(self.0.clone());
    }
}
impl tl::RemoteCall for TLRequest {
    type Return = TLObject;
}

struct PySenderInner {
    inner: Mutex<Option<Sender<transport::Full, mtp::Encrypted>>>,
    dc_id: i32,
    addr: SocketAddr,
}

#[derive(Clone)]
#[pyclass(from_py_object, name = "Sender", module = "telethon_mtsender")]
pub struct PySender(Arc<PySenderInner>);

#[pymethods]
impl PySender {
    #[new]
    fn new(dc_id: i32, address: String) -> PyResult<Self> {
        let addr = match address.parse::<SocketAddr>() {
            Ok(address) => address,
            Err(e) => {
                return Err(PyValueError::new_err(format!(
                    "Failed to parse address: {}",
                    e
                )));
            }
        };

        Ok(Self(Arc::new(PySenderInner {
            inner: Mutex::new(None),
            dc_id,
            addr,
        })))
    }

    #[getter]
    fn dc_id(&self) -> i32 {
        self.0.clone().dc_id
    }

    #[getter]
    fn addr(&self) -> String {
        self.0.clone().addr.to_string()
    }

    #[getter]
    fn auth_key(&self) -> Option<[u8; 256]> {
        let inner = self.0.clone();
        get_runtime().block_on(async move {
            inner
                .inner
                .lock()
                .await
                .as_ref()
                .as_mut()
                .map(|x| x.auth_key())
        })
    }

    #[pyo3(signature = (
        *,
        auth_key=None,
    ))]
    pub async fn connect(&self, auth_key: Option<[u8; 256]>) -> PyResult<()> {
        let inner = self.0.clone();
        get_runtime()
            .spawn(async move {
                let transport = transport::Full::new;
                let addr = || ServerAddr::Tcp {
                    address: inner.addr,
                };

                let sender = if let Some(auth_key) = auth_key {
                    connect_with_auth(transport(), addr(), auth_key)
                        .await
                        .map_err(|io_err| convert_invocation_error(InvocationError::Io(io_err)))?
                } else {
                    connect(transport(), addr())
                        .await
                        .map_err(convert_invocation_error)?
                };

                {
                    *inner.inner.lock().await = Some(sender);
                }

                Ok(())
            })
            .await
            .unwrap()
    }

    pub async fn invoke(&mut self, request: Vec<u8>) -> PyResult<Vec<u8>> {
        let inner = self.0.clone();
        get_runtime()
            .spawn(async move {
                match *inner.inner.lock().await {
                    None => Err(PyRuntimeError::new_err("Sender not connected.")),
                    Some(ref mut sender) => Ok(sender
                        .invoke(&TLRequest(request))
                        .await
                        .map_err(convert_invocation_error)?
                        .0),
                }
            })
            .await
            .unwrap()
    }

    pub async fn step(&self) -> PyResult<()> {
        todo!();
    }

    pub async fn disconnect(&self) -> PyResult<()> {
        todo!();
    }
}
