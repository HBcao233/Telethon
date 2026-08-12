use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3_async_runtimes::TaskLocals;
use pyo3_async_runtimes::tokio::{future_into_py, get_current_locals, get_runtime};

use grammers_mtsender::{
    ConnectionParams, SenderPool, SenderPoolFatHandle, UpdatesConfiguration, UpdatesReceiver,
};
use grammers_session::Session;

use crate::mtsender::errors::convert_invocation_error;
use crate::session::SessionLike;
use crate::session::updates::PyUpdateAndPeers;

struct SenderPoolInner {
    pool_task: Mutex<Option<JoinHandle<()>>>,

    handle: SenderPoolFatHandle,

    updates: AsyncMutex<Option<mpsc::Receiver<grammers_session::updates::UpdatesLike>>>,
    updates_receiver: AsyncMutex<Option<UpdatesReceiver>>,

    locals: PyOnceLock<TaskLocals>,
}

#[pyclass(name = "SenderPool", module = "telethon_mtsender")]
pub struct PySenderPool {
    session: Arc<SessionLike>,
    inner: Arc<SenderPoolInner>,
    catch_up: bool,

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

impl SenderPoolInner {
    async fn get_or_init_updates(
        &self,
        catch_up: bool,
    ) -> PyResult<tokio::sync::MutexGuard<'_, Option<UpdatesReceiver>>> {
        let mut guard = self.updates_receiver.lock().await;
        if guard.is_none() {
            let updates = self.updates.lock().await.take().ok_or_else(|| {
                PyRuntimeError::new_err("Unexpected UpdatesReceiver initialization error.")
            })?;
            let updates_receiver = UpdatesReceiver::create(
                self.handle.thin.clone(),
                self.handle.session.clone(),
                updates,
                UpdatesConfiguration { catch_up },
            )
            .await
            .map_err(|e| *e.downcast::<PyErr>().unwrap())?;
            *guard = Some(updates_receiver);
        }
        Ok(guard)
    }
}

impl PySenderPool {
    /// Ensure the tokio runtime is created
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
        catch_up=true,
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
        catch_up: bool,
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
                updates: AsyncMutex::new(Some(updates)),
                updates_receiver: AsyncMutex::new(None),
                locals: PyOnceLock::new(),
            }),
            catch_up,
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

    fn is_connected(&self) -> PyResult<bool> {
        let inner = self.inner.clone();
        Ok(inner
            .pool_task
            .lock()
            .map_err(|_| PyRuntimeError::new_err("the Mutex of pool_task poisoned."))?
            .is_some())
    }

    fn invoke_in_dc<'py>(
        &self,
        py: Python<'py>,
        dc_id: i32,
        body: PyBuffer<u8>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = self.get_locals(py)?;
        let body = body.to_vec(py)?;
        let inner = self.inner.clone();

        future_into_py(py, async move {
            inner
                .handle
                .raw_invoke_in_dc(dc_id, body)
                .await
                .map_err(convert_invocation_error)
        })
    }

    fn invoke<'py>(&self, py: Python<'py>, body: PyBuffer<u8>) -> PyResult<Bound<'py, PyAny>> {
        self.invoke_in_dc(py, self.home_dc_id()?, body)
    }

    fn disconnect<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let _ = self.get_locals(py)?;
        let inner = self.inner.clone();
        future_into_py(py, async move {
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
        })
    }

    fn pop_updates<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let _ = self.get_locals(py)?;
        let inner = self.inner.clone();
        let catch_up = self.catch_up;

        future_into_py(py, async move {
            let mut updates = inner.get_or_init_updates(catch_up).await?;
            let updates = updates.as_mut().unwrap();

            let (updates, users, chats) = updates.next().await.map_err(convert_invocation_error)?;
            Python::attach(|py| {
                Py::new(
                    py,
                    PyUpdateAndPeers {
                        updates,
                        users,
                        chats,
                    },
                )
            })
        })
    }

    /// Synchronize the updates state to the session.
    ///
    /// This is **not** automatically done on drop.
    fn sync_update_state<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let _ = self.get_locals(py)?;
        let inner = self.inner.clone();
        let catch_up = self.catch_up;

        future_into_py(py, async move {
            let updates = inner.get_or_init_updates(catch_up).await?;
            updates
                .as_ref()
                .unwrap()
                .sync_update_state()
                .await
                .map_err(|e| *e.downcast::<PyErr>().unwrap())
        })
    }
}
