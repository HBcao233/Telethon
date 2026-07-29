use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use pyo3::buffer::PyBuffer;
use pyo3::exceptions::{PyConnectionError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3_async_runtimes::TaskLocals;
use pyo3_async_runtimes::tokio::{get_current_locals, get_runtime};

use grammers_mtsender::{ConnectionParams, InvocationError, SenderPool, SenderPoolFatHandle};
use grammers_session::Session;
use grammers_session::types::{PeerId, PeerInfo, UpdateState, UpdatesState};
use grammers_session::updates::PrematureEndReason;
use grammers_tl_types as tl;
use tl::Deserializable;

use crate::mtsender::errors::convert_invocation_error;
use crate::session::updates::PyMessageBoxes;
use crate::session::{SessionLike, updates::UpdatesLike};

/// How long to wait after warning the user that the updates limit was exceeded.
const UPDATE_LIMIT_EXCEEDED_LOG_COOLDOWN: usize = 300;

// See https://core.telegram.org/method/updates.getChannelDifference.
const BOT_CHANNEL_DIFF_LIMIT: i32 = 100000;
const USER_CHANNEL_DIFF_LIMIT: i32 = 100;

/// Telegram sends `seq` equal to `0` when "it doesn't matter", so we use that value too.
const NO_SEQ: i32 = 0;

/// Non-update types like `messages.affectedMessages` can contain `pts` that should still be
/// processed. Because there's no `date`, a value of `0` is used as the sentinel value for
/// the `date` when constructing the dummy `Updates` (in order to handle them uniformly).
const NO_DATE: i32 = 0;

struct SenderPoolInner {
    pool_task: Mutex<Option<JoinHandle<()>>>,

    handle: SenderPoolFatHandle,

    updates: AsyncMutex<Option<mpsc::UnboundedReceiver<grammers_session::updates::UpdatesLike>>>,

    locals: PyOnceLock<TaskLocals>,
}

#[pyclass(name = "SenderPool", module = "telethon_mtsender")]
pub struct PySenderPool {
    session: Arc<SessionLike>,
    inner: Arc<SenderPoolInner>,
    should_get_state: bool,
    catch_up: bool,
    message_box: Option<Py<PyMessageBoxes>>,

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

impl PySenderPool {
    async fn invoke_in_dc<R: tl::RemoteCall>(
        &mut self,
        dc_id: i32,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        let inner = self.inner.clone();
        inner
            .handle
            .invoke_in_dc(dc_id, request.to_bytes())
            .await
            .and_then(|body| R::Return::from_bytes(&body).map_err(|e| e.into()))
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
                locals: PyOnceLock::new(),
            }),
            should_get_state: false,
            catch_up,
            message_box: None,
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

    #[getter]
    fn _message_box(&self) -> Option<Py<PyMessageBoxes>> {
        match &self.message_box {
            Some(x) => Some(Python::attach(|py| x.clone_ref(py))),
            None => None,
        }
    }

    fn is_connected(&self) -> PyResult<bool> {
        let inner = self.inner.clone();
        Ok(inner
            .pool_task
            .lock()
            .map_err(|_| PyRuntimeError::new_err("the Mutex of pool_task poisoned."))?
            .is_some())
    }

    #[pyo3(name = "invoke_in_dc")]
    async fn py_invoke_in_dc(&self, dc_id: i32, body: PyBuffer<u8>) -> PyResult<Vec<u8>> {
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

    #[pyo3(name = "invoke")]
    async fn py_invoke(&self, body: PyBuffer<u8>) -> PyResult<Vec<u8>> {
        self.py_invoke_in_dc(self.home_dc_id()?, body).await
    }

    async fn disconnect(&self) -> PyResult<()> {
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

    async fn pop_updates(&mut self) -> PyResult<Vec<UpdatesLike>> {
        let session = self.session.clone();
        let home_dc_id = self.home_dc_id()?;

        let state = if self.catch_up {
            Some(session.updates_state().await?)
        } else {
            // If the user doesn't want to bother with catching up on previous update, start with
            // pristine state instead.
            None
        };
        let message_box = Python::attach(|py| {
            Ok::<_, PyErr>(
                if self.message_box.is_none() {
                    let mb = PyMessageBoxes::from(state);

                    self.message_box.insert(Py::new(py, mb)?)
                } else {
                    self.message_box.as_ref().unwrap()
                }
                .clone_ref(py),
            )
        })?;

        if self.should_get_state {
            self.should_get_state = false;
            match self
                .invoke_in_dc(home_dc_id, &tl::functions::updates::GetState {})
                .await
            {
                Ok(tl::enums::updates::State::State(state)) => {
                    session
                        .set_update_state(UpdateState::All(UpdatesState {
                            pts: state.pts,
                            qts: state.qts,
                            date: state.date,
                            seq: state.seq,
                            channels: Vec::new(),
                        }))
                        .await?;
                    Python::attach(|py| message_box.borrow_mut(py).0.set_state(state));
                }
                Err(_err) => {
                    // The account may no longer actually be logged in, or it can rarely fail.
                    // `message_box` will try to correct its state as updates arrive.
                }
            }
        }

        if let Some(request) = Python::attach(|py| message_box.borrow(py).0.get_difference()) {
            let response = self
                .invoke_in_dc(home_dc_id, &request)
                .await
                .map_err(convert_invocation_error)?;
            let (updates, users, chats) =
                Python::attach(|py| message_box.borrow_mut(py).0.apply_difference(response));
            return Ok(vec![
                grammers_session::updates::UpdatesLike::Updates(tl::enums::Updates::Updates(
                    tl::types::Updates {
                        updates: updates.into_iter().map(|(update, _state)| update).collect(),
                        users,
                        chats,
                        date: NO_DATE,
                        seq: NO_SEQ,
                    },
                ))
                .into(),
            ]);
        }

        loop {
            if let Some(gd) = Python::attach(|py| message_box.borrow(py).0.get_channel_difference())
            {
                let request =
                    match prepare_channel_difference(gd, session.clone(), &message_box).await? {
                        Some(req) => req,
                        None => break,
                    };

                let maybe_response = self.invoke_in_dc(home_dc_id, &request).await;

                let response = match maybe_response {
                    Ok(r) => r,
                    Err(e) if e.is("PERSISTENT_TIMESTAMP_OUTDATED") => {
                        // According to Telegram's docs:
                        // "Channel internal replication issues, try again later (treat this like an RPC_CALL_FAIL)."
                        // We can treat this as "empty difference" and not update the local pts.
                        // Then this same call will be retried when another gap is detected or timeout expires.
                        //
                        // Another option would be to literally treat this like an RPC_CALL_FAIL and retry after a few
                        // seconds, but if Telegram is having issues it's probably best to wait for it to send another
                        // update (hinting it may be okay now) and retry then.
                        //
                        // This is a bit hacky because MessageBox doesn't really have a way to "not update" the pts.
                        // Instead we manually extract the previously-known pts and use that.
                        log::warn!(
                            "Getting difference for channel updates caused PersistentTimestampOutdated; ending getting difference prematurely until server issues are resolved"
                        );
                        {
                            Python::attach(|py| {
                                message_box.borrow_mut(py).0.end_channel_difference(
                                    PrematureEndReason::TemporaryServerIssues,
                                )
                            });
                        }
                        continue;
                    }
                    Err(e) if e.is("CHANNEL_PRIVATE") => {
                        log::info!(
                            "Account is now banned so we can no longer fetch updates with request: {:?}",
                            request
                        );
                        {
                            Python::attach(|py| {
                                message_box
                                    .borrow_mut(py)
                                    .0
                                    .end_channel_difference(PrematureEndReason::Banned)
                            });
                        }
                        continue;
                    }
                    Err(InvocationError::Rpc(rpc_error)) if rpc_error.code == 500 => {
                        log::warn!("Telegram is having internal issues: {:#?}", rpc_error);
                        {
                            Python::attach(|py| {
                                message_box.borrow_mut(py).0.end_channel_difference(
                                    PrematureEndReason::TemporaryServerIssues,
                                )
                            });
                        }
                        continue;
                    }
                    Err(e) => return Err(convert_invocation_error(e)),
                };

                let (updates, users, chats) = Python::attach(|py| {
                    message_box
                        .borrow_mut(py)
                        .0
                        .apply_channel_difference(response)
                });
                return Ok(vec![
                    grammers_session::updates::UpdatesLike::Updates(tl::enums::Updates::Updates(
                        tl::types::Updates {
                            updates: updates.into_iter().map(|(update, _state)| update).collect(),
                            users,
                            chats,
                            date: NO_DATE,
                            seq: NO_SEQ,
                        },
                    ))
                    .into(),
                ]);
            } else {
                break;
            }
        }

        let mut updates_lock = self.inner.updates.lock().await;
        let rx = updates_lock
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("Updates receiver has been closed or taken"))?;

        let mut results = Vec::new();

        match rx.recv().await {
            Some(update) => {
                results.push(update.into());
            }
            None => {
                return Err(PyConnectionError::new_err(
                    "The senderPool has been disconnected.".to_string(),
                ));
            }
        }

        while let Ok(update) = rx.try_recv() {
            results.push(update.into());
            if results.len() >= UPDATE_LIMIT_EXCEEDED_LOG_COOLDOWN {
                break;
            }
        }

        Ok(results)
    }

    /// Synchronize the updates state to the session.
    ///
    /// This is **not** automatically done on drop.
    async fn sync_update_state(&self) -> PyResult<()> {
        match &self.message_box {
            Some(message_box) => {
                self.session
                    .clone()
                    .set_update_state(UpdateState::All(Python::attach(|py| {
                        message_box.borrow(py).0.session_state()
                    })))
                    .await
            }
            None => Err(PyRuntimeError::new_err(
                "message_box has not been initialized.",
            )),
        }
    }
}

async fn prepare_channel_difference(
    mut request: tl::functions::updates::GetChannelDifference,
    session: Arc<SessionLike>,
    message_box: &Py<PyMessageBoxes>,
) -> PyResult<Option<tl::functions::updates::GetChannelDifference>> {
    let id = match &request.channel {
        tl::enums::InputChannel::Channel(channel) => PeerId::channel_unchecked(channel.channel_id),
        _ => unreachable!(),
    };

    if let Some(PeerInfo::Channel {
        id,
        auth: Some(auth),
        ..
    }) = session.peer(id).await?
    {
        request.channel = tl::enums::InputChannel::Channel(tl::types::InputChannel {
            channel_id: id,
            access_hash: auth.hash(),
        });
        request.limit = if session
            .peer(PeerId::self_user())
            .await?
            .map(|user| match user {
                PeerInfo::User { bot, .. } => bot.unwrap_or(false),
                _ => false,
            })
            .unwrap_or(false)
        {
            BOT_CHANNEL_DIFF_LIMIT
        } else {
            USER_CHANNEL_DIFF_LIMIT
        };
        log::trace!("requesting {:?}", request);
        Ok(Some(request))
    } else {
        log::warn!(
            "cannot getChannelDifference for {:?} as we're missing its hash",
            id
        );
        Python::attach(|py| {
            message_box
                .borrow_mut(py)
                .0
                .end_channel_difference(PrematureEndReason::Banned)
        });
        Ok(None)
    }
}
