use pyo3::PyTypeInfo;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyType;

use grammers_session::types::{ChannelKind, PeerAuth, PeerId, PeerInfo};

/// Sentinel value used to represent the self-user
/// when its true `PeerId` is unknown.
///
/// Per <https://core.telegram.org/api/bots/ids>:
/// > a bot API dialog ID ranges from -4000000000000 to 1099511627775
///
/// This value is not intended to be visible or persisted,
/// so it can be changed as needed in the future.
pub const SELF_USER_ID: i64 = 1 << 40;

/// Sentinel value used to represent empty chats.
///
/// Per <https://core.telegram.org/api/bots/ids>:
/// > \[…] transformed range for bot API chat dialog IDs is -999999999999 to -1 inclusively
/// >
/// > \[…] transformed range for bot API channel dialog IDs is -1997852516352 to -1000000000001 inclusively
///
/// `chat_id` parameters are in Telegram's API use the bare identifier,
/// so there's no empty constructor,
/// but it can be mimicked by picking the value in the correct range hole.
/// This value is closer to "channel with ID 0" than "chat with ID 0",
/// but there's no distinct `-0` integer,
/// and channels have a proper constructor for empty already.
// const EMPTY_CHAT_ID: i64 = -1000000000000;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[pyclass(from_py_object, name = "PeerId", module = "grammers.sessions")]
pub struct PyPeerId(pub i64);

#[pymethods]
impl PyPeerId {
    #[new]
    pub fn new(id: PeerIdLike) -> PyResult<Self> {
        let id = id.0;
        if (1 <= id && id <= 0xffffffffff)
            || id == SELF_USER_ID
            || (-999999999999 <= id && id <= -1)
            || (-1997852516352 <= id && id <= -1000000000001)
            || (-4000000000000 <= id && id <= -2002147483649)
        {
            Ok(Self(id))
        } else {
            Err(PyValueError::new_err("Invalid peer id"))
        }
    }

    /// Creates a peer identity for the currently-logged-in user or bot account.
    ///
    /// Internally, this will use a special sentinel value outside of any valid Bot API Dialog ID range.
    #[staticmethod]
    pub fn self_user() -> PyResult<Self> {
        Ok(Self(SELF_USER_ID))
    }

    /// Creates a peer identity for a user or bot account.
    #[staticmethod]
    pub fn user(id: i64) -> PyResult<Self> {
        if !(1 <= id && id <= 0xffffffffff) {
            Err(PyValueError::new_err("user ID out of range"))
        } else {
            Ok(Self(id))
        }
    }

    /// Creates a peer identity for a small group chat.
    #[staticmethod]
    pub fn chat(id: i64) -> PyResult<Self> {
        // https://core.telegram.org/api/bots/ids#chat-ids
        if !(1 <= id && id <= 999999999999) {
            Err(PyValueError::new_err("chat ID out of range"))
        } else {
            Ok(Self(-id))
        }
    }

    /// Creates a peer identity for a broadcast channel, megagroup, gigagroup or monoforum.
    #[staticmethod]
    pub fn channel(id: i64) -> PyResult<Self> {
        // https://core.telegram.org/api/bots/ids#supergroup-channel-ids and #monoforum-ids
        if !((1 <= id && id <= 997852516352) || (1002147483649 <= id && id <= 3000000000000)) {
            Err(PyValueError::new_err("channel ID out of range"))
        } else {
            Ok(Self(-(1000000000000 + id)))
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "PeerId({}, kind={})",
            self.bot_api_dialog_id(),
            self.kind().__repr__()
        )
    }

    /// Peer kind.
    #[getter]
    pub fn kind(&self) -> PyPeerKind {
        if 1 <= self.0 && self.0 <= 0xffffffffff {
            PyPeerKind::User
        } else if self.0 == SELF_USER_ID {
            PyPeerKind::UserSelf
        } else if -999999999999 <= self.0 && self.0 <= -1 {
            PyPeerKind::Chat
        } else if -1997852516352 <= self.0 && self.0 <= -1000000000001
            || (-4000000000000 <= self.0 && self.0 <= -2002147483649)
        {
            PyPeerKind::Channel
        } else {
            unreachable!()
        }
    }

    /// Returns the identity using the Bot API Dialog ID format.
    ///
    /// Will return an arbitrary value if [`Self::kind`] is [`PeerKind::UserSelf`].
    /// This value should not be relied on and may change between releases.
    #[getter]
    pub fn bot_api_dialog_id(&self) -> i64 {
        self.0
    }

    fn __int__(&self) -> i64 {
        self.0
    }

    fn __index__(&self) -> i64 {
        self.0
    }

    /// Unpacked peer identifier. Panics if [`Self::kind`] is [`PeerKind::UserSelf`].
    #[getter]
    pub fn bare_id(&self) -> PyResult<i64> {
        Ok(match self.kind() {
            PyPeerKind::User => self.0,
            PyPeerKind::UserSelf => return Err(PyValueError::new_err("self-user ID not known")),
            PyPeerKind::Chat => -self.0,
            PyPeerKind::Channel => -self.0 - 1000000000000,
        })
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(other_peer) = other.cast::<PyPeerId>() {
            return Ok(self.0 == other_peer.borrow().0);
        }
        if let Ok(other_int) = other.extract::<i64>() {
            return Ok(self.0 == other_int);
        }
        Ok(false)
    }

    fn __hash__(&self) -> i64 {
        self.0
    }
}

impl From<PeerId> for PyPeerId {
    fn from(x: PeerId) -> Self {
        Self(x.bot_api_dialog_id_unchecked())
    }
}

impl From<PyPeerId> for PeerId {
    fn from(x: PyPeerId) -> Self {
        match x.kind() {
            PyPeerKind::UserSelf => Self::self_user(),
            _ => Self::from_bot_api_dialog_id(x.0).unwrap(),
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeerIdLike(pub i64);
impl<'a, 'py> FromPyObject<'a, 'py> for PeerIdLike {
    type Error = PyErr;

    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(v) = ob.extract::<i64>() {
            return Ok(Self(v));
        }
        if let Ok(v) = ob.extract::<PyPeerId>() {
            return Ok(Self(v.0));
        }
        let cls_name = ob.get_type().qualname()?;
        Err(PyTypeError::new_err(format!(
            "expected int or PeerId, got '{}'",
            cls_name
        )))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[pyclass(
    from_py_object,
    name = "PeerKind",
    module = "grammers.sessions",
    eq,
    eq_int,
    frozen,
    hash
)]
pub enum PyPeerKind {
    /// The peer identity belongs to a [`tl.types.User`]. May also represent [`PeerKind.UserSelf`].
    User,
    /// The peer identity belongs to a user with its [`tl.types.User.is_self`] flag set to `true`.
    UserSelf,
    /// The peer identity belongs to a [`tl.types.Chat`] or one of its derivatives.
    Chat,
    /// The peer identity belongs to a [`tl.types.Channel`] or one of its derivatives.
    Channel,
}

#[pymethods]
impl PyPeerKind {
    fn __repr__(&self) -> String {
        match self {
            Self::User => "PeerKind.User",
            Self::UserSelf => "PeerKind.UserSelf",
            Self::Chat => "PeerKind.Chat",
            Self::Channel => "PeerKind.Channel",
        }
        .to_string()
    }

    fn __int__(&self) -> i64 {
        *self as i64
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[pyclass(
    from_py_object,
    name = "PeerAuth",
    module = "grammers.sessions",
    eq,
    frozen,
    hash
)]
pub struct PyPeerAuth(pub i64);

#[pymethods]
impl PyPeerAuth {
    #[new]
    pub fn new(access_hash: i64) -> Self {
        Self(access_hash)
    }

    fn __repr__(&self) -> String {
        format!("PeerAuth({})", self.0)
    }

    fn __int__(&self) -> i64 {
        self.0
    }

    fn __index__(&self) -> i64 {
        self.0
    }
}

impl Default for PyPeerAuth {
    /// Returns the ambient authority to authorize peers only when Telegram considers it valid.
    ///
    /// The internal representation uses `0` to signal the ambient authority,
    /// although this might happen to be the actual witness used by some peers.
    fn default() -> Self {
        Self(0)
    }
}

impl From<PeerAuth> for PyPeerAuth {
    fn from(x: PeerAuth) -> Self {
        Self(x.hash())
    }
}

impl From<PyPeerAuth> for PeerAuth {
    fn from(x: PyPeerAuth) -> Self {
        Self::from_hash(x.0)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeerAuthLike(pub PyPeerAuth);
impl std::ops::Deref for PeerAuthLike {
    type Target = PyPeerAuth;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a, 'py> FromPyObject<'a, 'py> for PeerAuthLike {
    type Error = PyErr;

    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(v) = ob.extract::<i64>() {
            return Ok(Self(PyPeerAuth(v)));
        }
        if let Ok(v) = ob.extract::<PyPeerAuth>() {
            return Ok(Self(v));
        }

        let cls_name = ob.get_type().qualname()?;
        Err(PyTypeError::new_err(format!(
            "peer_auth expected an int or PeerAuth, got '{}'",
            cls_name
        )))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[pyclass(
    from_py_object,
    name = "ChannelKind",
    module = "grammers.sessions",
    eq,
    eq_int,
    frozen,
    hash
)]
pub enum PyChannelKind {
    /// Value used for a channel with its [`tl.types.Channel.broadcast`] flag set to `true`.
    Broadcast = 1,
    /// Value used for a channel with its [`tl.types.Channel.megagroup`] flag set to `true`.
    Megagroup,
    /// Value used for a channel with its [`tl.types.Channel.gigagroup`] flag set to `true`.
    Gigagroup,
}

#[pymethods]
impl PyChannelKind {
    fn __repr__(&self) -> String {
        match self {
            Self::Broadcast => "ChannelKind.Broadcast",
            Self::Megagroup => "ChannelKind.Megagroup",
            Self::Gigagroup => "ChannelKind.Gigagroup",
        }
        .to_string()
    }

    fn __int__(&self) -> i64 {
        *self as i64
    }
}

impl From<ChannelKind> for PyChannelKind {
    fn from(x: ChannelKind) -> Self {
        match x {
            ChannelKind::Broadcast => Self::Broadcast,
            ChannelKind::Megagroup => Self::Megagroup,
            ChannelKind::Gigagroup => Self::Gigagroup,
        }
    }
}

impl From<PyChannelKind> for ChannelKind {
    fn from(x: PyChannelKind) -> Self {
        match x {
            PyChannelKind::Broadcast => Self::Broadcast,
            PyChannelKind::Megagroup => Self::Megagroup,
            PyChannelKind::Gigagroup => Self::Gigagroup,
        }
    }
}

/// An exploded peer reference along with any known useful information about the peer.
#[derive(Clone, Debug)]
#[pyclass(
    skip_from_py_object,
    name = "PeerInfo",
    module = "grammers.sessions",
    subclass
)]
pub struct PyPeerInfo {}

#[pymethods]
impl PyPeerInfo {
    #[classattr]
    #[pyo3(name = "User")]
    fn user(py: Python<'_>) -> Py<PyType> {
        PyPeerInfoUser::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "Chat")]
    fn chat(py: Python<'_>) -> Py<PyType> {
        PyPeerInfoChat::type_object(py).unbind()
    }

    #[classattr]
    #[pyo3(name = "Channel")]
    fn channel(py: Python<'_>) -> Py<PyType> {
        PyPeerInfoChannel::type_object(py).unbind()
    }
}

#[derive(Clone, PartialEq, Eq)]
#[pyclass(from_py_object, name = "PeerInfoUser", module = "grammers.sessions", extends = PyPeerInfo, eq)]
pub struct PyPeerInfoUser {
    /// Bare user identifier.
    ///
    /// Despite being `i64`, Telegram only uses strictly positive values.
    id: i64,
    /// Non-ambient authority bound to both the user itself and the session.
    #[pyo3(get, set)]
    auth: Option<PyPeerAuth>,
    /// Whether this user represents a bot or not.
    #[pyo3(get, set)]
    bot: Option<bool>,
    /// Whether this user represents the logged-in user authorized by this session or not.
    #[pyo3(get, set)]
    is_self: Option<bool>,
}

#[pymethods]
impl PyPeerInfoUser {
    #[new]
    #[pyo3(signature = (id, auth=None, bot=None, is_self=None))]
    fn new(
        id: i64,
        auth: Option<PeerAuthLike>,
        bot: Option<bool>,
        is_self: Option<bool>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let _ = PyPeerId::user(id)?;
        let base = PyClassInitializer::from(PyPeerInfo {});
        let x = Self {
            id,
            auth: auth.map(|x| x.0),
            bot,
            is_self,
        };
        Ok(base.add_subclass(x))
    }

    #[getter]
    fn id(&self) -> PyPeerId {
        PyPeerId::user(self.id).unwrap()
    }

    #[setter(id)]
    fn set_id(&mut self, id: PeerIdLike) -> PyResult<()> {
        let _ = PyPeerId::user(id.0)?;
        self.id = id.0;
        Ok(())
    }

    #[getter]
    fn access_hash(&self) -> Option<PyPeerAuth> {
        self.auth
    }

    #[setter(access_hash)]
    fn set_access_hash(&mut self, access_hash: Option<PyPeerAuth>) -> PyResult<()> {
        self.auth = access_hash;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "PeerInfo.User(id={}, auth={}, bot={}, is_self={})",
            self.id().__repr__(),
            match self.auth {
                Some(x) => x.__repr__(),
                None => "None".to_string(),
            },
            match self.bot {
                Some(true) => "True",
                Some(false) => "False",
                None => "None",
            },
            match self.is_self {
                Some(true) => "True",
                Some(false) => "False",
                None => "None",
            },
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
#[pyclass(from_py_object, name = "PeerInfoChat", module = "grammers.sessions", extends = PyPeerInfo, eq)]
pub struct PyPeerInfoChat {
    id: i64,
}

#[pymethods]
impl PyPeerInfoChat {
    #[new]
    fn new(id: i64) -> PyResult<PyClassInitializer<Self>> {
        let _ = PyPeerId::chat(id)?;
        let base = PyClassInitializer::from(PyPeerInfo {});
        Ok(base.add_subclass(Self { id }))
    }

    #[getter]
    fn id(&self) -> PyPeerId {
        PyPeerId::chat(self.id).unwrap()
    }

    #[setter(id)]
    fn set_id(&mut self, id: PeerIdLike) -> PyResult<()> {
        let _ = PyPeerId::chat(id.0)?;
        self.id = id.0;
        Ok(())
    }

    #[getter]
    fn access_hash(&self) -> Option<PyPeerAuth> {
        None
    }

    #[setter(access_hash)]
    fn set_access_hash(&mut self, _access_hash: Option<PyPeerAuth>) -> PyResult<()> {
        Err(PyTypeError::new_err(
            "PeerInfoChat can't be set access_hash.",
        ))
    }

    fn __repr__(&self) -> String {
        format!("PeerInfo.Chat(id={})", self.id().__repr__(),)
    }
}

#[derive(Clone, PartialEq, Eq)]
#[pyclass(from_py_object, name = "PeerInfoChannel", module = "grammers.sessions", extends = PyPeerInfo, eq)]
pub struct PyPeerInfoChannel {
    /// Bare channel identifier.
    ///
    /// Note that the HTTP Bot API prefixes this identifier with `-100` to signal that it is a channel,
    /// but the true value used by Telegram's API is always strictly-positive.
    #[pyo3(get, set)]
    id: i64,
    /// Non-ambient authority bound to both the user itself and the session.
    #[pyo3(get, set)]
    auth: Option<PyPeerAuth>,
    /// Channel kind, useful to determine what the possible permissions on it are.
    #[pyo3(get, set)]
    kind: Option<PyChannelKind>,
}

#[pymethods]
impl PyPeerInfoChannel {
    #[new]
    #[pyo3(signature = (id, auth=None, kind=None))]
    fn new(
        id: i64,
        auth: Option<PeerAuthLike>,
        kind: Option<PyChannelKind>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let _ = PyPeerId::channel(id)?;
        let base = PyClassInitializer::from(PyPeerInfo {});
        let x = Self {
            id,
            auth: auth.map(|x| x.0),
            kind,
        };
        Ok(base.add_subclass(x))
    }

    #[getter]
    fn id(&self) -> PyPeerId {
        PyPeerId::channel(self.id).unwrap()
    }

    #[setter(id)]
    fn set_id(&mut self, id: PeerIdLike) -> PyResult<()> {
        let _ = PyPeerId::channel(id.0)?;
        self.id = id.0;
        Ok(())
    }

    #[getter]
    fn access_hash(&self) -> Option<PyPeerAuth> {
        self.auth
    }

    #[setter(access_hash)]
    fn set_access_hash(&mut self, access_hash: Option<PyPeerAuth>) -> PyResult<()> {
        self.auth = access_hash;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "PeerInfo.Channel(id={}, auth={}, kind={})",
            self.id().__repr__(),
            match self.auth {
                Some(x) => x.__repr__(),
                None => "None".to_string(),
            },
            match self.kind {
                Some(x) => x.__repr__(),
                None => "None".to_string(),
            },
        )
    }
}

// for use on the Rust side
pub enum PeerInfoLike {
    User {
        /// Bare user identifier.
        ///
        /// Despite being `i64`, Telegram only uses strictly positive values.
        id: i64,
        /// Non-ambient authority bound to both the user itself and the session.
        auth: Option<PyPeerAuth>,
        /// Whether this user represents a bot or not.
        bot: Option<bool>,
        /// Whether this user represents the logged-in user authorized by this session or not.
        is_self: Option<bool>,
    },
    Chat {
        /// Bare chat identifier.
        ///
        /// Note that the HTTP Bot API negates this identifier to signal that it is a chat,
        /// but the true value used by Telegram's API is always strictly-positive.
        id: i64,
    },
    Channel {
        /// Bare channel identifier.
        ///
        /// Note that the HTTP Bot API prefixes this identifier with `-100` to signal that it is a channel,
        /// but the true value used by Telegram's API is always strictly-positive.
        id: i64,
        /// Non-ambient authority bound to both the user itself and the session.
        auth: Option<PyPeerAuth>,
        /// Channel kind, useful to determine what the possible permissions on it are.
        kind: Option<PyChannelKind>,
    },
}

impl<'a, 'py> FromPyObject<'a, 'py> for PeerInfoLike {
    type Error = PyErr;
    fn extract(ob: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(x) = ob.extract::<PyPeerInfoUser>() {
            return Ok(Self::User {
                id: x.id,
                auth: x.auth,
                bot: x.bot,
                is_self: x.is_self,
            });
        }
        if let Ok(x) = ob.extract::<PyPeerInfoChat>() {
            return Ok(Self::Chat { id: x.id });
        }
        if let Ok(x) = ob.extract::<PyPeerInfoChannel>() {
            return Ok(Self::Channel {
                id: x.id,
                auth: x.auth,
                kind: x.kind,
            });
        }
        let cls_name = ob.get_type().qualname()?;
        Err(PyTypeError::new_err(format!(
            "expected PeerInfo, got '{}'.",
            cls_name
        )))
    }
}

impl<'py> IntoPyObject<'py> for PeerInfoLike {
    type Target = PyPeerInfo;
    type Output = Bound<'py, PyPeerInfo>;
    type Error = PyErr;
    fn into_pyobject(self, py: Python<'py>) -> PyResult<Bound<'py, PyPeerInfo>> {
        match self {
            Self::User {
                id,
                auth,
                bot,
                is_self,
            } => Ok(Bound::new(
                py,
                PyPeerInfoUser::new(id, auth.map(PeerAuthLike), bot, is_self)?,
            )?
            .into_super()),
            Self::Chat { id } => Ok(Bound::new(py, PyPeerInfoChat::new(id)?)?.into_super()),
            Self::Channel { id, auth, kind } => Ok(Bound::new(
                py,
                PyPeerInfoChannel::new(id, auth.map(PeerAuthLike), kind)?,
            )?
            .into_super()),
        }
    }
}

impl From<PeerInfo> for PeerInfoLike {
    fn from(x: PeerInfo) -> Self {
        match x {
            PeerInfo::User {
                id,
                auth,
                bot,
                is_self,
            } => Self::User {
                id,
                auth: auth.map(Into::into),
                bot,
                is_self,
            },
            PeerInfo::Chat { id } => Self::Chat { id },
            PeerInfo::Channel { id, auth, kind } => Self::Channel {
                id,
                auth: auth.map(Into::into),
                kind: kind.map(Into::into),
            },
        }
    }
}

impl From<PeerInfoLike> for PeerInfo {
    fn from(x: PeerInfoLike) -> Self {
        match x {
            PeerInfoLike::User {
                id,
                auth,
                bot,
                is_self,
            } => Self::User {
                id,
                auth: auth.map(Into::into),
                bot,
                is_self,
            },
            PeerInfoLike::Chat { id } => Self::Chat { id },
            PeerInfoLike::Channel { id, auth, kind } => Self::Channel {
                id,
                auth: auth.map(Into::into),
                kind: kind.map(Into::into),
            },
        }
    }
}

impl PeerInfoLike {
    pub fn id(&self) -> PyPeerId {
        match self {
            Self::User { id, .. } => PyPeerId::user(*id).unwrap(),
            Self::Chat { id } => PyPeerId::chat(*id).unwrap(),
            Self::Channel { id, .. } => PyPeerId::channel(*id).unwrap(),
        }
    }

    pub fn auth(&self) -> Option<PyPeerAuth> {
        match self {
            Self::User { auth, .. } => *auth,
            Self::Chat { .. } => None,
            Self::Channel { auth, .. } => *auth,
        }
    }

    pub fn to_ref(&self) -> PyPeerRef {
        match self {
            Self::User { id, auth, .. } => PyPeerRef {
                id: PyPeerId::user(*id).unwrap(),
                auth: auth.unwrap_or_default(),
            },
            Self::Chat { id } => PyPeerRef {
                id: PyPeerId::chat(*id).unwrap(),
                auth: PyPeerAuth::default(),
            },
            Self::Channel { id, auth, .. } => PyPeerRef {
                id: PyPeerId::channel(*id).unwrap(),
                auth: auth.unwrap_or_default(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[pyclass(
    from_py_object,
    name = "PeerRef",
    module = "grammers.sessions",
    subclass
)]
pub struct PyPeerRef {
    /// The peer identity.
    #[pyo3(get, set)]
    pub id: PyPeerId,
    /// The authority bound to both the sibling identity and the session of the logged-in user.
    #[pyo3(get)]
    pub auth: PyPeerAuth,
}

impl PyPeerRef {
    pub fn id(&self) -> PyPeerId {
        self.id
    }

    pub fn auth(&self) -> PyPeerAuth {
        self.auth
    }
}

#[pymethods]
impl PyPeerRef {
    #[new]
    fn new(id: PeerIdLike, auth: PeerAuthLike) -> Self {
        Self {
            id: PyPeerId(id.0),
            auth: auth.0,
        }
    }

    #[setter(auth)]
    fn set_auth(&mut self, auth: PeerAuthLike) {
        self.auth = auth.0;
    }

    #[getter]
    fn access_hash(&self) -> PyPeerAuth {
        self.auth
    }

    #[getter]
    fn kind(&self) -> PyPeerKind {
        self.id().kind()
    }

    fn __repr__(&self) -> String {
        format!(
            "PeerRef(id={}, auth={})",
            self.id.__repr__(),
            self.auth.__repr__()
        )
    }
}
