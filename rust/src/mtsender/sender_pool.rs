use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3_async_runtimes::TaskLocals;
use pyo3_async_runtimes::tokio::{get_current_locals, get_runtime};

use grammers_mtsender::{ConnectionParams, SenderPool, SenderPoolFatHandle};
use grammers_session::Session;
use grammers_session::updates::UpdatesLike;
use tokio::sync::mpsc;

use crate::mtsender::errors::convert_invocation_error;
use crate::session::SessionLike;

struct SenderPoolInner {
    pool_task: Mutex<Option<JoinHandle<()>>>,

    handle: SenderPoolFatHandle,

    // TODO
    updates: Mutex<Option<mpsc::UnboundedReceiver<UpdatesLike>>>,

    locals: PyOnceLock<TaskLocals>,
}

#[pyclass(name = "SenderPool", module = "telethon_mtsender")]
pub struct PySenderPool {
    session: Arc<SessionLike>,
    inner: Arc<SenderPoolInner>,

    #[pyo3(get)]
    api_id: i32,
    #[pyo3(get)]
    use_ipv6: bool,
    #[pyo3(get)]
    device_model: String,
    #[pyo3(get)]
    system_version: String,
    #[pyo3(get)]
    app_version: String,
    #[pyo3(get)]
    system_lang_code: String,
    #[pyo3(get)]
    lang_code: String,
}

impl PySenderPool {
    fn get_locals<'py>(&self, py: Python<'py>) -> PyResult<TaskLocals> {
        let locals = self
            .inner
            .clone()
            .locals
            .get_or_try_init(py, || Ok::<_, PyErr>(get_current_locals(py)?))?
            .clone();
        self.session.clone().init_locals(py, locals.clone());
        Ok(locals)
    }
}

#[pymethods]
impl PySenderPool {
    #[new]
    #[pyo3(signature = (
        session,
        api_id,
        *,
        use_ipv6=false,
        device_model=String::new(),
        system_version=String::new(),
        app_version=String::new(),
        system_lang_code=String::new(),
        lang_code=String::new(),
    ))]
    fn new(
        session: SessionLike,
        api_id: i32,
        use_ipv6: bool,
        device_model: String,
        system_version: String,
        app_version: String,
        system_lang_code: String,
        lang_code: String,
    ) -> Self {
        let mut config = ConnectionParams::default();
        if use_ipv6 {
            config.use_ipv6 = true;
        }

        if !device_model.is_empty() {
            config.device_model = device_model.clone();
        }
        if !system_version.is_empty() {
            config.system_version = system_version.clone();
        }
        if !app_version.is_empty() {
            config.app_version = app_version.clone();
        }
        if !system_lang_code.is_empty() {
            config.system_lang_code = system_lang_code.clone();
        }
        if !lang_code.is_empty() {
            config.lang_code = lang_code.clone();
        }

        let session = Arc::new(session);
        let SenderPool {
            runner,
            updates,
            handle,
        } = SenderPool::with_configuration(session.clone(), api_id, config);
        let pool_task = get_runtime().spawn(runner.run());

        Self {
            session,
            inner: Arc::new(SenderPoolInner {
                handle,
                pool_task: Mutex::new(Some(pool_task)),
                updates: Mutex::new(Some(updates)),
                locals: PyOnceLock::new(),
            }),
            api_id,
            use_ipv6,
            device_model,
            system_version,
            app_version,
            system_lang_code,
            lang_code,
        }
    }

    #[getter]
    fn session(&self, py: Python<'_>) -> Py<PyAny> {
        self.session.clone().get_inner(py)
    }

    #[getter]
    fn home_dc_id(&self) -> PyResult<i32> {
        self.session.clone().home_dc_id()
    }

    pub fn is_connected(&self) -> PyResult<bool> {
        let inner = self.inner.clone();
        Ok(inner
            .pool_task
            .lock()
            .map_err(|_| PyRuntimeError::new_err("the Mutex of pool_task poisoned."))?
            .is_some())
    }

    pub async fn invoke_in_dc(&self, dc_id: i32, body: PyBuffer<u8>) -> PyResult<Vec<u8>> {
        let body = Python::attach(|py| {
            let _ = self.get_locals(py)?;

            body.to_vec(py)
        })?;

        let inner = self.inner.clone();
        inner
            .handle
            .invoke_in_dc(dc_id, body)
            .await
            .map_err(convert_invocation_error)
    }

    pub async fn invoke(&self, body: PyBuffer<u8>) -> PyResult<Vec<u8>> {
        self.invoke_in_dc(self.home_dc_id()?, body).await
    }

    pub async fn disconnect(&self) -> PyResult<()> {
        let _ = Python::attach(|py| self.get_locals(py))?;

        let inner = self.inner.clone();
        inner.handle.quit();

        let pool_task = inner
            .pool_task
            .lock()
            .map_err(|_| PyRuntimeError::new_err("the Mutex of pool_task poisoned."))?
            .take();
        if let Some(pool_task) = pool_task {
            pool_task
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        }

        Ok(())
    }
}
