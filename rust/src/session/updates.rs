use pyo3::PyTypeInfo;
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyType;

use grammers_session::updates::State;
use grammers_tl_types as tl;
use tl::{Deserializable, Serializable};

use crate::DeserializeError;

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
