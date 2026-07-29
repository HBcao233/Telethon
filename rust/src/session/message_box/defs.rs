use std::time::{Duration, Instant};

use pyo3::PyTypeInfo;
use pyo3::buffer::PyBuffer;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::PyType;

use grammers_session::updates::{PrematureEndReason, State};
use grammers_tl_types as tl;
use tl::{Deserializable, Serializable};

use crate::DeserializeError;

create_exception!("telethon._impl.session", GapError, PyException);

#[derive(Clone)]
#[pyclass(from_py_object, name = "Instant")]
pub struct PyInstant(pub(super) Instant);

#[pymethods]
impl PyInstant {
    #[staticmethod]
    pub fn now() -> Self {
        Self(Instant::now())
    }

    pub fn duration_since(&self, earlier: PyInstant) -> Duration {
        self.0.duration_since(earlier.0)
    }

    pub fn checked_duration_since(&self, earlier: PyInstant) -> Option<Duration> {
        self.0.checked_duration_since(earlier.0)
    }

    pub fn saturating_duration_since(&self, earlier: PyInstant) -> Duration {
        self.0.saturating_duration_since(earlier.0)
    }

    pub fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }

    pub fn __add__(&self, other: Duration) -> Self {
        Self(self.0 + other)
    }

    pub fn __sub__(&self, other: Duration) -> Self {
        Self(self.0 - other)
    }
}

#[pyclass(name = "MessageBox", module = "telethon._impl.session", subclass)]
pub struct PyMessageBox {}

#[pymethods]
impl PyMessageBox {
    #[classattr]
    #[pyo3(name = "Common")]
    fn common(py: Python<'_>) -> Py<PyType> {
        PyCommon::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "Secondary")]
    fn secondary(py: Python<'_>) -> Py<PyType> {
        PySecondary::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "Channel")]
    fn channel(py: Python<'_>) -> Py<PyType> {
        PyChannel::type_object(py).unbind()
    }
}

#[derive(Clone)]
#[pyclass(from_py_object, name = "Common", module = "telethon._impl.session", extends = PyMessageBox)]
pub struct PyCommon {
    #[pyo3(get, set)]
    pts: i32,
}

#[pymethods]
impl PyCommon {
    #[new]
    fn new(pts: i32) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyMessageBox {}).add_subclass(Self { pts })
    }
}

#[derive(Clone)]
#[pyclass(from_py_object, name = "Secondary", module = "telethon._impl.session", extends = PyMessageBox)]
pub struct PySecondary {
    #[pyo3(get, set)]
    qts: i32,
}

#[pymethods]
impl PySecondary {
    #[new]
    fn new(qts: i32) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyMessageBox {}).add_subclass(Self { qts })
    }
}

#[derive(Clone)]
#[pyclass(from_py_object, name = "Channel", module = "telethon._impl.session", extends = PyMessageBox)]
pub struct PyChannel {
    #[pyo3(get, set)]
    channel_id: i64,
    #[pyo3(get, set)]
    pts: i32,
}

#[pymethods]
impl PyChannel {
    #[new]
    fn new(channel_id: i64, pts: i32) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyMessageBox {}).add_subclass(Self { channel_id, pts })
    }
}

/// The message box and pts value that uniquely identifies the message-related update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessageBox {
    /// Account-wide persistent timestamp.
    ///
    /// This includes private conversations (one-to-one) and small group chats.
    Common { pts: i32 },
    /// Account-wide secondary persistent timestamp.
    ///
    /// This includes only certain bot updates and secret one-to-one chats.
    Secondary { qts: i32 },
    /// Channel-specific persistent timestamp.
    ///
    /// This includes "megagroup", "broadcast" and "supergroup" channels.
    Channel { channel_id: i64, pts: i32 },
}

impl<'a, 'py> FromPyObject<'a, 'py> for MessageBox {
    type Error = PyErr;

    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(v) = ob.extract::<PyCommon>() {
            return Ok(Self::Common { pts: v.pts });
        }
        if let Ok(v) = ob.extract::<PySecondary>() {
            return Ok(Self::Secondary { qts: v.qts });
        }
        if let Ok(v) = ob.extract::<PyChannel>() {
            return Ok(Self::Channel {
                channel_id: v.channel_id,
                pts: v.pts,
            });
        }

        let cls_name = ob.get_type().qualname()?;
        Err(PyTypeError::new_err(format!(
            "expected int or PeerId, got '{}'",
            cls_name
        )))
    }
}

impl<'py> IntoPyObject<'py> for MessageBox {
    type Target = PyMessageBox;
    type Output = Bound<'py, PyMessageBox>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Bound<'py, PyMessageBox>> {
        match self {
            Self::Common { pts } => Ok(Bound::new(py, PyCommon::new(pts))?.into_super()),
            Self::Secondary { qts } => Ok(Bound::new(py, PySecondary::new(qts))?.into_super()),
            Self::Channel { channel_id, pts } => {
                Ok(Bound::new(py, PyChannel::new(channel_id, pts))?.into_super())
            }
        }
    }
}

impl From<MessageBox> for grammers_session::updates::MessageBox {
    fn from(x: MessageBox) -> Self {
        match x {
            MessageBox::Common { pts } => Self::Common { pts },
            MessageBox::Secondary { qts } => Self::Secondary { qts },
            MessageBox::Channel { channel_id, pts } => Self::Channel { channel_id, pts },
        }
    }
}

impl From<grammers_session::updates::MessageBox> for MessageBox {
    fn from(x: grammers_session::updates::MessageBox) -> Self {
        use grammers_session::updates::MessageBox as M;
        match x {
            M::Common { pts } => Self::Common { pts },
            M::Secondary { qts } => Self::Secondary { qts },
            M::Channel { channel_id, pts } => Self::Channel { channel_id, pts },
        }
    }
}

/// Update state, up to and including the update it is a part of.
///
/// When using `catch_up` with a client, all updates with a
/// state containing a [`MessageBox`] higher than this one will be fetched.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[pyclass(from_py_object, name = "State", module = "telethon._impl.session")]
pub struct PyState {
    /// Last date state assigned by Telegram, which changes somewhat arbitrarily.
    #[pyo3(get, set)]
    pub date: i32,
    /// Last sequence state assigned by Telegram, which changes somewhat arbitrarily.
    #[pyo3(get, set)]
    pub seq: i32,
    /// The particular message box change if the update pertains to a message-related event sequence.
    #[pyo3(get, set)]
    pub message_box: Option<MessageBox>,
}

#[pymethods]
impl PyState {
    #[new]
    fn new(date: i32, seq: i32, message_box: Option<MessageBox>) -> Self {
        Self {
            date,
            seq,
            message_box,
        }
    }
}

impl From<PyState> for State {
    fn from(x: PyState) -> Self {
        Self {
            date: x.date,
            seq: x.seq,
            message_box: x.message_box.map(Into::into),
        }
    }
}

impl From<State> for PyState {
    fn from(x: State) -> Self {
        Self {
            date: x.date,
            seq: x.seq,
            message_box: x.message_box.map(Into::into),
        }
    }
}

#[derive(Clone)]
#[pyclass(
    from_py_object,
    name = "UpdateAndPeers",
    module = "telethon._impl.session"
)]
pub struct PyUpdateAndPeers {
    pub(crate) updates: Vec<(tl::enums::Update, State)>,
    pub(crate) users: Vec<tl::enums::User>,
    pub(crate) chats: Vec<tl::enums::Chat>,
}

#[pymethods]
impl PyUpdateAndPeers {
    #[new]
    fn new(
        updates: Vec<(PyBuffer<u8>, PyState)>,
        users: Vec<PyBuffer<u8>>,
        chats: Vec<PyBuffer<u8>>,
    ) -> PyResult<Self> {
        let (updates, users, chats) = Python::attach(|py| {
            let updates = updates
                .into_iter()
                .map(|(update, state)| {
                    let update = update.to_vec(py)?;
                    let update = tl::enums::Update::from_bytes(&update)
                        .map_err(|e| DeserializeError::new_err(e.to_string()))?;
                    Ok((update, State::from(state)))
                })
                .collect::<Result<Vec<_>, PyErr>>()?;
            let users = users
                .into_iter()
                .map(|user| {
                    let user = user.to_vec(py)?;
                    let user = tl::enums::User::from_bytes(&user)
                        .map_err(|e| DeserializeError::new_err(e.to_string()))?;
                    Ok(user)
                })
                .collect::<Result<Vec<_>, PyErr>>()?;
            let chats = chats
                .into_iter()
                .map(|chat| {
                    let chat = chat.to_vec(py)?;
                    let chat = tl::enums::Chat::from_bytes(&chat)
                        .map_err(|e| DeserializeError::new_err(e.to_string()))?;
                    Ok(chat)
                })
                .collect::<Result<Vec<_>, PyErr>>()?;
            Ok::<_, PyErr>((updates, users, chats))
        })?;
        Ok(Self {
            updates,
            users,
            chats,
        })
    }

    #[getter]
    fn updates(&self) -> Vec<(Vec<u8>, PyState)> {
        self.updates
            .iter()
            .map(|(update, state)| (update.to_bytes(), state.clone().into()))
            .collect()
    }

    #[getter]
    fn users(&self) -> Vec<Vec<u8>> {
        self.users.iter().map(|user| user.to_bytes()).collect()
    }

    #[getter]
    fn chats(&self) -> Vec<Vec<u8>> {
        self.chats.iter().map(|chat| chat.to_bytes()).collect()
    }
}

#[derive(Clone)]
#[pyclass(
    from_py_object,
    name = "UpdatesLike",
    module = "telethon._impl.session",
    subclass
)]
pub struct PyUpdatesLike {}

#[pymethods]
impl PyUpdatesLike {
    #[classattr]
    #[pyo3(name = "Updates")]
    fn updates(py: Python<'_>) -> Py<PyType> {
        PyUpdates::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "ShortSentMessage")]
    fn short_sent_message(py: Python<'_>) -> Py<PyType> {
        PyShortSentMessage::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "AffectedMessages")]
    fn affected_messages(py: Python<'_>) -> Py<PyType> {
        PyAffectedMessages::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "AffectedChannelMessages")]
    fn affected_channel_messages(py: Python<'_>) -> Py<PyType> {
        PyAffectedChannelMessages::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "InvitedUsers")]
    fn invited_users(py: Python<'_>) -> Py<PyType> {
        PyInvitedUsers::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "ChatInviteJoinResult")]
    fn chat_invite_join_result(py: Python<'_>) -> Py<PyType> {
        PyChatInviteJoinResult::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "ConnectionClosed")]
    fn connection_closed(py: Python<'_>) -> Py<PyType> {
        PyConnectionClosed::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "MalformedUpdates")]
    fn malformed_updates(py: Python<'_>) -> Py<PyType> {
        PyMalformedUpdates::type_object(py).unbind()
    }
}

/// The usual variant, received passively from Telegram.
#[derive(Clone)]
#[pyclass(skip_from_py_object, name = "Updates", module = "telethon._impl.session", extends = PyUpdatesLike)]
pub struct PyUpdates(tl::enums::Updates);

impl PyUpdates {
    fn from(updates: tl::enums::Updates) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyUpdatesLike {}).add_subclass(Self(updates))
    }
}

#[pymethods]
impl PyUpdates {
    #[new]
    pub fn new(updates: PyBuffer<u8>) -> PyResult<PyClassInitializer<Self>> {
        let updates = Python::attach(|py| updates.to_vec(py))?;
        let updates = tl::enums::Updates::from_bytes(&updates)
            .map_err(|e| DeserializeError::new_err(e.to_string()))?;
        Ok(Self::from(updates))
    }

    #[getter]
    fn updates(&self) -> Vec<u8> {
        self.0.to_bytes()
    }
}

/// Special-case for short-sent messages,
/// where the request is also needed to construct a complete update.
#[derive(Clone)]
#[pyclass(skip_from_py_object, name = "ShortSentMessage", module = "telethon._impl.session", extends = PyUpdatesLike)]
pub struct PyShortSentMessage {
    /// The request that triggered the short update.
    request: tl::functions::messages::SendMessage,
    /// The incomplete update caused by the request.
    update: tl::types::UpdateShortSentMessage,
}

impl PyShortSentMessage {
    fn from(
        request: tl::functions::messages::SendMessage,
        update: tl::types::UpdateShortSentMessage,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyUpdatesLike {}).add_subclass(Self { request, update })
    }
}

#[pymethods]
impl PyShortSentMessage {
    #[new]
    pub fn new(request: PyBuffer<u8>, update: PyBuffer<u8>) -> PyResult<PyClassInitializer<Self>> {
        let (request, update) =
            Python::attach(|py| Ok::<_, PyErr>((request.to_vec(py)?, update.to_vec(py)?)))?;
        let request = tl::functions::messages::SendMessage::from_bytes(&request)
            .map_err(|e| DeserializeError::new_err(e.to_string()))?;
        let update = tl::types::UpdateShortSentMessage::from_bytes(&update)
            .map_err(|e| DeserializeError::new_err(e.to_string()))?;
        Ok(Self::from(request, update))
    }

    #[getter]
    fn request(&self) -> Vec<u8> {
        self.request.to_bytes()
    }

    #[getter]
    fn update(&self) -> Vec<u8> {
        self.update.to_bytes()
    }
}

/// Special-case for requests that affect some messages.
#[derive(Clone)]
#[pyclass(skip_from_py_object, name = "AffectedMessages", extends = PyUpdatesLike)]
pub struct PyAffectedMessages(tl::types::messages::AffectedMessages);

impl PyAffectedMessages {
    fn from(update: tl::types::messages::AffectedMessages) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyUpdatesLike {}).add_subclass(Self(update))
    }
}

#[pymethods]
impl PyAffectedMessages {
    #[new]
    pub fn new(update: PyBuffer<u8>) -> PyResult<PyClassInitializer<Self>> {
        let update = Python::attach(|py| update.to_vec(py))?;
        let update = tl::types::messages::AffectedMessages::from_bytes(&update)
            .map_err(|e| DeserializeError::new_err(e.to_string()))?;
        Ok(Self::from(update))
    }

    #[getter]
    fn update(&self) -> Vec<u8> {
        self.0.to_bytes()
    }
}

/// Special-case for channel-specific requests that affect messages (e.g.
/// `channels.deleteMessages`). The `channel_id` is needed so the `pts` can
/// be applied to the correct `Key::Channel` instead of `Key::Common`.
#[derive(Clone)]
#[pyclass(skip_from_py_object, name = "AffectedChannelMessages", extends = PyUpdatesLike)]
pub struct PyAffectedChannelMessages {
    affected: tl::types::messages::AffectedMessages,
    #[pyo3(get)]
    channel_id: i64,
    #[pyo3(get)]
    message_ids: Vec<i32>,
}

impl PyAffectedChannelMessages {
    fn from(
        affected: tl::types::messages::AffectedMessages,
        channel_id: i64,
        message_ids: Vec<i32>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyUpdatesLike {}).add_subclass(Self {
            affected,
            channel_id,
            message_ids,
        })
    }
}

#[pymethods]
impl PyAffectedChannelMessages {
    #[new]
    pub fn new(
        affected: PyBuffer<u8>,
        channel_id: i64,
        message_ids: Vec<i32>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let affected = Python::attach(|py| affected.to_vec(py))?;
        let affected = tl::types::messages::AffectedMessages::from_bytes(&affected)
            .map_err(|e| DeserializeError::new_err(e.to_string()))?;
        Ok(Self::from(affected, channel_id, message_ids))
    }

    #[getter]
    fn affected(&self) -> Vec<u8> {
        self.affected.to_bytes()
    }
}

/// Special-case for requests that lead to users being invited.
#[derive(Clone)]
#[pyclass(skip_from_py_object, name = "InvitedUsers", extends = PyUpdatesLike)]
pub struct PyInvitedUsers(tl::types::messages::InvitedUsers);

impl PyInvitedUsers {
    fn from(update: tl::types::messages::InvitedUsers) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyUpdatesLike {}).add_subclass(Self(update))
    }
}

#[pymethods]
impl PyInvitedUsers {
    #[new]
    pub fn new(update: PyBuffer<u8>) -> PyResult<PyClassInitializer<Self>> {
        let update = Python::attach(|py| update.to_vec(py))?;
        let update = tl::types::messages::InvitedUsers::from_bytes(&update)
            .map_err(|e| DeserializeError::new_err(e.to_string()))?;
        Ok(Self::from(update))
    }

    #[getter]
    fn update(&self) -> Vec<u8> {
        self.0.to_bytes()
    }
}

/// Special-case for join requests that contain updates inside a `ChatInviteJoinResultOk`.
#[derive(Clone)]
#[pyclass(skip_from_py_object, name = "ChatInviteJoinResult", extends = PyUpdatesLike)]
pub struct PyChatInviteJoinResult(tl::types::messages::ChatInviteJoinResultOk);

impl PyChatInviteJoinResult {
    fn from(update: tl::types::messages::ChatInviteJoinResultOk) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyUpdatesLike {}).add_subclass(Self(update))
    }
}

#[pymethods]
impl PyChatInviteJoinResult {
    #[new]
    pub fn new(update: PyBuffer<u8>) -> PyResult<PyClassInitializer<Self>> {
        let update = Python::attach(|py| update.to_vec(py))?;
        let update = tl::types::messages::ChatInviteJoinResultOk::from_bytes(&update)
            .map_err(|e| DeserializeError::new_err(e.to_string()))?;
        Ok(Self::from(update))
    }

    #[getter]
    fn update(&self) -> Vec<u8> {
        self.0.to_bytes()
    }
}

/// Indicates that the connection was closed and had to be recreated.
/// This may mean that an update gap now exists and should be resolved.
#[derive(Clone)]
#[pyclass(skip_from_py_object, name = "ConnectionClosed", extends = PyUpdatesLike)]
pub struct PyConnectionClosed {}

#[pymethods]
impl PyConnectionClosed {
    #[new]
    fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyUpdatesLike {}).add_subclass(Self {})
    }
}

/// Indicates that passively-received updates were malformed.
/// Either the constructor identifier was unexpected (possibly a stale channel
/// update) or the updates were cut short during deserialization (very unlikely).
/// This should be treated as a gap.
#[derive(Clone)]
#[pyclass(skip_from_py_object, name = "MalformedUpdates", extends = PyUpdatesLike)]
pub struct PyMalformedUpdates {}

#[pymethods]
impl PyMalformedUpdates {
    #[new]
    fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyUpdatesLike {}).add_subclass(Self {})
    }
}

#[derive(FromPyObject, IntoPyObject)]
pub(crate) enum UpdatesLike {
    Updates(Py<PyUpdates>),
    ShortSentMessage(Py<PyShortSentMessage>),
    AffectedMessages(Py<PyAffectedMessages>),
    AffectedChannelMessages(Py<PyAffectedChannelMessages>),
    InvitedUsers(Py<PyInvitedUsers>),
    ChatInviteJoinResult(Py<PyChatInviteJoinResult>),
    ConnectionClosed(Py<PyConnectionClosed>),
    MalformedUpdates(Py<PyMalformedUpdates>),
}

impl From<grammers_session::updates::UpdatesLike> for UpdatesLike {
    fn from(updates: grammers_session::updates::UpdatesLike) -> Self {
        use grammers_session::updates::UpdatesLike as U;
        Python::attach(|py| match updates {
            U::Updates(x) => Self::Updates(Py::new(py, PyUpdates::from(x)).unwrap()),
            U::ShortSentMessage { request, update } => Self::ShortSentMessage(
                Py::new(py, PyShortSentMessage::from(request, update)).unwrap(),
            ),
            U::AffectedMessages(x) => {
                Self::AffectedMessages(Py::new(py, PyAffectedMessages::from(x)).unwrap())
            }
            U::AffectedChannelMessages {
                affected,
                channel_id,
                message_ids,
            } => Self::AffectedChannelMessages(
                Py::new(
                    py,
                    PyAffectedChannelMessages::from(affected, channel_id, message_ids),
                )
                .unwrap(),
            ),
            U::InvitedUsers(x) => Self::InvitedUsers(Py::new(py, PyInvitedUsers::from(x)).unwrap()),
            U::ChatInviteJoinResult(x) => {
                Self::ChatInviteJoinResult(Py::new(py, PyChatInviteJoinResult::from(x)).unwrap())
            }
            U::ConnectionClosed => {
                Self::ConnectionClosed(Py::new(py, PyConnectionClosed::new()).unwrap())
            }
            U::MalformedUpdates => {
                Self::MalformedUpdates(Py::new(py, PyMalformedUpdates::new()).unwrap())
            }
        })
    }
}

impl From<UpdatesLike> for grammers_session::updates::UpdatesLike {
    fn from(updates: UpdatesLike) -> Self {
        Python::attach(|py| match updates {
            UpdatesLike::Updates(x) => Self::Updates(x.borrow(py).0.clone()),
            UpdatesLike::ShortSentMessage(x) => {
                let x = x.borrow(py);
                Self::ShortSentMessage {
                    request: x.request.clone(),
                    update: x.update.clone(),
                }
            }
            UpdatesLike::AffectedMessages(x) => Self::AffectedMessages(x.borrow(py).0.clone()),
            UpdatesLike::AffectedChannelMessages(x) => {
                let x = x.borrow(py);
                Self::AffectedChannelMessages {
                    affected: x.affected.clone(),
                    channel_id: x.channel_id,
                    message_ids: x.message_ids.clone(),
                }
            }
            UpdatesLike::InvitedUsers(x) => Self::InvitedUsers(x.borrow(py).0.clone()),
            UpdatesLike::ChatInviteJoinResult(x) => {
                Self::ChatInviteJoinResult(x.borrow(py).0.clone())
            }
            UpdatesLike::ConnectionClosed(_) => Self::ConnectionClosed,
            UpdatesLike::MalformedUpdates(_) => Self::MalformedUpdates,
        })
    }
}

/// Reason for calling [`MessageBoxes::end_channel_difference`].
#[derive(Clone, Debug)]
#[pyclass(
    from_py_object,
    name = "PrematureEndReason",
    module = "telethon._impl.session"
)]
pub enum PyPrematureEndReason {
    /// The channel difference failed to be fetched for a temporary reasons.
    ///
    /// The channel state must be kept,
    /// and getting its difference retried in the future.
    TemporaryServerIssues,
    /// The logged-in account has been banned from the channel and won't
    /// be able to fetch the difference in the future.
    ///
    /// The channel state will be removed,
    /// and getting its difference should not be retried in the future.
    Banned,
}

impl From<PyPrematureEndReason> for PrematureEndReason {
    fn from(x: PyPrematureEndReason) -> Self {
        match x {
            PyPrematureEndReason::TemporaryServerIssues => {
                PrematureEndReason::TemporaryServerIssues
            }
            PyPrematureEndReason::Banned => PrematureEndReason::Banned,
        }
    }
}
