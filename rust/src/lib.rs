mod crypto;
mod mtsender;
mod session;

pub use mtsender::{
    DeserializeError, DroppedError, PyRpcError, PySender, PySenderPool, TransportError,
};
pub use session::{PySession, SessionLike};

#[pyo3::pymodule]
mod _rs {
    #[pymodule_export]
    #[allow(non_upper_case_globals)]
    const __version__: &str = &env!("CARGO_PKG_VERSION");

    #[pymodule_export]
    #[allow(non_upper_case_globals)]
    const __doc__: &str = &"Python bindings for grammers-mtsender library";

    #[pymodule_export]
    use crate::crypto::calculate_2fa;

    #[pymodule_export]
    use crate::crypto::check_p_and_g;


    #[pymodule_export]
    use crate::session::peer::PyPeerId;

    #[pymodule_export]
    use crate::session::peer::PyPeerKind;

    #[pymodule_export]
    use crate::session::peer::PyPeerAuth;

    #[pymodule_export]
    use crate::session::peer::PyChannelKind;

    #[pymodule_export]
    use crate::session::peer::PyPeerInfo;

    #[pymodule_export]
    use crate::session::peer::PyPeerRef;

    #[pymodule_export]
    use crate::session::types::PyDcOption;

    #[pymodule_export]
    use crate::session::types::PyChannelState;

    #[pymodule_export]
    use crate::session::types::PyUpdateState;

    #[pymodule_export]
    use crate::session::types::PyUpdatesState;

    #[pymodule_export]
    use crate::session::PySession;


    #[pymodule_export]
    use super::PyRpcError;

    #[pymodule_export]
    use super::DroppedError;

    #[pymodule_export]
    use super::DeserializeError;

    #[pymodule_export]
    use super::TransportError;

    #[pymodule_export]
    use super::PySender;

    #[pymodule_export]
    use super::PySenderPool;
}
