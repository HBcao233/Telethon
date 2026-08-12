use std::sync::Arc;

use pyo3::exceptions::{PyNotImplementedError, PyTypeError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyType;
use pyo3_async_runtimes::tokio::get_current_locals;
use pyo3_async_runtimes::{TaskLocals, into_future_with_locals};

use grammers_session::types::{DcOption, PeerId, PeerInfo, UpdateState, UpdatesState};
use grammers_session::{BoxFuture, Session};

use super::peer::{PeerIdLike, PeerInfoLike, PyPeerId, PyPeerInfo};
use super::types::{PyDcOption, PyUpdateState, PyUpdatesState, UpdateStateLike};

/// ABC Session
#[derive(Clone)]
#[pyclass(
    skip_from_py_object,
    name = "Session",
    module = "telethon_mtsender",
    subclass,
    dict
)]
pub struct PySession {}

#[pymethods]
impl PySession {
    #[new]
    fn new() -> Self {
        Self {}
    }

    #[classmethod]
    fn __init_subclass__(cls: &Bound<'_, PyType>) -> PyResult<()> {
        let cls_dict = cls.getattr("__dict__")?;
        let required_methods = [
            "home_dc_id",
            "set_home_dc_id",
            "dc_option",
            "set_home_dc_id",
            "peer",
            "cache_peer",
            "updates_state",
            "set_update_state",
        ];

        for method in required_methods {
            if !cls_dict.contains(method)? {
                return Err(PyTypeError::new_err(format!(
                    "Can't instantiate abstract class {} without an implementation for abstract method '{}'",
                    cls.name()?,
                    method,
                )));
            }
        }
        Ok(())
    }

    fn home_dc_id(&self) -> PyResult<i32> {
        Err(PyNotImplementedError::new_err(
            "Session subclasses must implement home_dc_id()",
        ))
    }

    #[allow(unused_variables)]
    #[pyo3(signature = (dc_id))]
    fn set_home_dc_id(&self, dc_id: i32) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(
            "Session subclasses must implement set_home_dc_id()",
        ))
    }

    #[allow(unused_variables)]
    #[pyo3(signature = (dc_id))]
    fn dc_option(&self, dc_id: i32) -> PyResult<Option<PyDcOption>> {
        Err(PyNotImplementedError::new_err(
            "Session subclasses must implement dc_option()",
        ))
    }

    #[allow(unused_variables)]
    #[pyo3(signature = (dc_option))]
    fn set_dc_option(&self, dc_option: Py<PyDcOption>) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(
            "Session subclasses must implement set_dc_option()",
        ))
    }

    #[allow(unused_variables)]
    #[pyo3(signature = (peer))]
    fn peer(&self, peer: PeerIdLike) -> PyResult<Option<PyPeerInfo>> {
        Err(PyNotImplementedError::new_err(
            "Session subclasses must implement peer()",
        ))
    }

    #[allow(unused_variables)]
    #[pyo3(signature = (peer_info))]
    fn cache_peer(&self, peer_info: Py<PyPeerInfo>) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(
            "Session subclasses must implement cache_peer()",
        ))
    }

    #[pyo3(signature = ())]
    fn updates_state(&self) -> PyResult<Py<PyUpdatesState>> {
        Err(PyNotImplementedError::new_err(
            "Session subclasses must implement updates_state()",
        ))
    }

    #[allow(unused_variables)]
    #[pyo3(signature = (update))]
    fn set_update_state(&self, update: PyUpdateState) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(
            "Session subclasses must implement set_update_state()",
        ))
    }
}

struct SessionLikeInner {
    inner: Py<PyAny>,
    locals: PyOnceLock<TaskLocals>,
}

#[derive(Clone)]
pub struct SessionLike(Arc<SessionLikeInner>);

impl<'a, 'py> FromPyObject<'a, 'py> for SessionLike {
    type Error = PyErr;

    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        if ob.is_instance_of::<PySession>() {
            Ok(SessionLike::new(Bound::clone(&ob).unbind()))
        } else {
            let cls_name = ob.get_type().qualname()?;
            Err(PyTypeError::new_err(format!(
                "session expected a Session, got {}",
                cls_name
            )))
        }
    }
}

impl SessionLike {
    pub fn new(inner: Py<PyAny>) -> Self {
        Self(Arc::new(SessionLikeInner {
            inner,
            locals: PyOnceLock::new(),
        }))
    }

    pub fn get_inner(&self, py: Python<'_>) -> Py<PyAny> {
        self.0.clone().inner.clone_ref(py)
    }

    pub fn cls_name(&self, py: Python<'_>) -> PyResult<String> {
        self.0
            .clone()
            .inner
            .bind(py)
            .get_type()
            .qualname()?
            .extract::<String>()
    }

    fn get_locals<'py>(&self, py: Python<'py>) -> PyResult<TaskLocals> {
        self.0
            .clone()
            .locals
            .get_or_try_init(py, || Ok(get_current_locals(py)?))
            .cloned()
    }

    pub(crate) fn init_locals<'py>(&self, py: Python<'py>, value: TaskLocals) {
        self.0.clone().locals.get_or_init(py, || value);
    }

    fn into_future(
        &self,
        awaitable: Py<PyAny>,
    ) -> impl Future<Output = PyResult<Py<PyAny>>> + Send {
        let this = self.clone();
        async move {
            Python::attach(|py| {
                let locals = this.get_locals(py)?;
                into_future_with_locals(&locals, awaitable.into_bound(py))
            })?
            .await
        }
    }
}

impl Session for SessionLike {
    type Error = PyErr;

    fn home_dc_id(&self) -> PyResult<i32> {
        Python::attach(|py| {
            let res = self.0.inner.bind(py).call_method0("home_dc_id")?;
            res.extract().map_err(|e| {
                let cls_name = match Python::attach(|py| self.cls_name(py)) {
                    Ok(name) => name,
                    Err(e) => return e,
                };
                PyTypeError::new_err(format!("{}.home_dc_id(): {}", cls_name, e))
            })
        })
    }

    fn set_home_dc_id(&self, dc_id: i32) -> BoxFuture<'_, PyResult<()>> {
        let this = self.clone();

        Box::pin(async move {
            let coro = Python::attach(|py| {
                this.0
                    .inner
                    .bind(py)
                    .call_method1("set_home_dc_id", (dc_id,))
                    .map(|x| x.unbind())
            })?;
            this.into_future(coro).await?;

            Ok(())
        })
    }

    fn dc_option(&self, dc_id: i32) -> PyResult<Option<DcOption>> {
        Python::attach(|py| {
            let res = self.0.inner.bind(py).call_method1("dc_option", (dc_id,))?;
            let res: Option<PyDcOption> = res.extract().map_err(|e| {
                let cls_name = match Python::attach(|py| self.cls_name(py)) {
                    Ok(name) => name,
                    Err(e) => return e,
                };
                PyTypeError::new_err(format!("{}.dc_option(): {}", cls_name, e))
            })?;

            Ok(res.map(Into::into))
        })
    }

    fn set_dc_option(&self, dc_option: &DcOption) -> BoxFuture<'_, PyResult<()>> {
        let dc_option: PyDcOption = dc_option.clone().into();
        let this = self.clone();

        Box::pin(async move {
            let coro = Python::attach(|py| {
                this.0
                    .inner
                    .bind(py)
                    .call_method1("set_dc_option", (dc_option,))
                    .map(|x| x.unbind())
            })?;
            this.into_future(coro).await?;

            Ok(())
        })
    }

    fn peer(&self, peer: PeerId) -> BoxFuture<'_, PyResult<Option<PeerInfo>>> {
        let peer: PyPeerId = peer.into();
        let this = self.clone();

        Box::pin(async move {
            let coro = Python::attach(|py| {
                this.0
                    .inner
                    .bind(py)
                    .call_method1("peer", (peer,))
                    .map(|x| x.unbind())
            })?;
            let res = this.into_future(coro).await?;
            let res: Option<PeerInfoLike> = Python::attach(|py| res.extract(py)).map_err(|e| {
                let cls_name = match Python::attach(|py| self.cls_name(py)) {
                    Ok(name) => name,
                    Err(e) => return e,
                };
                PyTypeError::new_err(format!("{}.peer(): {}", cls_name, e))
            })?;

            Ok(res.map(Into::into))
        })
    }

    fn cache_peer(&self, peer_info: &PeerInfo) -> BoxFuture<'_, PyResult<()>> {
        let peer_info: PeerInfoLike = peer_info.clone().into();
        let this = self.clone();

        Box::pin(async move {
            let coro = Python::attach(|py| {
                this.0
                    .inner
                    .bind(py)
                    .call_method1("cache_peer", (peer_info,))
                    .map(|x| x.unbind())
            })?;
            this.into_future(coro).await?;

            Ok(())
        })
    }

    fn updates_state(&self) -> BoxFuture<'_, PyResult<UpdatesState>> {
        let this = self.clone();

        Box::pin(async move {
            let coro = Python::attach(|py| {
                this.0
                    .inner
                    .bind(py)
                    .call_method0("updates_state")
                    .map(|x| x.unbind())
            })?;
            let res = this.into_future(coro).await?;
            let res: PyUpdatesState = Python::attach(|py| Ok::<_, PyErr>(res.extract(py)?))
                .map_err(|e| {
                    let cls_name = match Python::attach(|py| self.cls_name(py)) {
                        Ok(name) => name,
                        Err(e) => return e,
                    };
                    PyTypeError::new_err(format!("{}.updates_state(): {}", cls_name, e))
                })?;
            Ok(res.into())
        })
    }

    fn set_update_state(&self, update: UpdateState) -> BoxFuture<'_, PyResult<()>> {
        let update: UpdateStateLike = update.into();
        let this = self.clone();

        Box::pin(async move {
            let coro = Python::attach(|py| {
                let update = update.into_pyobject(py)?;
                this.0
                    .inner
                    .bind(py)
                    .call_method1("set_update_state", (update,))
                    .map(|x| x.unbind())
            })?;
            this.into_future(coro).await?;

            Ok(())
        })
    }
}
