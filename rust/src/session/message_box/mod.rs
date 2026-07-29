mod defs;

use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;

use grammers_session::types::UpdatesState;
use grammers_session::updates::MessageBoxes;
use grammers_tl_types as tl;
use tl::{Deserializable, Serializable};

use crate::DeserializeError;
use crate::session::types::{PyUpdateState, PyUpdatesState};
pub(crate) use defs::UpdatesLike;
pub use defs::{
    GapError, PyInstant, PyMessageBox, PyPrematureEndReason, PyState, PyUpdateAndPeers,
    PyUpdatesLike,
};

#[pyclass(name = "MessageBoxes", module = "telethon._impl.session")]
pub struct PyMessageBoxes(pub(crate) MessageBoxes);

impl PyMessageBoxes {
    pub(crate) fn from(state: Option<UpdatesState>) -> Self {
        PyMessageBoxes(match state {
            None => MessageBoxes::new(),
            Some(s) => MessageBoxes::load(s),
        })
    }
}

#[pymethods]
impl PyMessageBoxes {
    // ========================================
    // Creation, querying, and setting base state.
    // ========================================

    #[new]
    #[pyo3(signature = (state=None))]
    pub(crate) fn new(state: Option<Py<PyUpdatesState>>) -> Self {
        PyMessageBoxes(match state {
            None => MessageBoxes::new(),
            Some(s) => MessageBoxes::load(Python::attach(|py| s.borrow(py).clone()).into()),
        })
    }

    fn session_state(&self) -> PyResult<Py<PyUpdatesState>> {
        let update = self.0.session_state().into();
        Python::attach(|py| {
            Py::new(
                py,
                PyClassInitializer::from(PyUpdateState {}).add_subclass(update),
            )
        })
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn check_deadlines(&mut self) -> PyInstant {
        PyInstant(self.0.check_deadlines())
    }

    fn set_state(&mut self, state: PyBuffer<u8>) -> PyResult<()> {
        let state = Python::attach(|py| state.to_vec(py))?;
        use tl::deserialize::Deserializable;
        let state = tl::types::updates::State::from_bytes(&state)
            .map_err(|e| DeserializeError::new_err(e.to_string()))?;
        self.0.set_state(state);
        Ok(())
    }

    fn try_set_channel_state(&mut self, id: i64, pts: i32) {
        self.0.try_set_channel_state(id, pts);
    }

    // ========================================
    // "Normal" updates flow (processing and detection of gaps).
    // ========================================

    fn process_updates(&mut self, updates: UpdatesLike) -> PyResult<PyUpdateAndPeers> {
        let (updates, users, chats) = self
            .0
            .process_updates(updates.into())
            .map_err(|_| GapError::new_err(()))?;
        Ok(PyUpdateAndPeers {
            updates,
            users,
            chats,
        })
    }

    // ========================================
    // Getting and applying account difference.
    // ========================================

    fn get_difference(&self) -> Option<Vec<u8>> {
        match self.0.get_difference() {
            None => None,
            Some(request) => Some(request.to_bytes()), // tl::functions::updates::GetDifference
        }
    }

    fn apply_difference(&mut self, difference: PyBuffer<u8>) -> PyResult<PyUpdateAndPeers> {
        let difference = Python::attach(|py| difference.to_vec(py))?;
        let difference = tl::enums::updates::Difference::from_bytes(&difference)
            .map_err(|e| DeserializeError::new_err(e.to_string()))?;
        let (updates, users, chats) = self.0.apply_difference(difference);
        Ok(PyUpdateAndPeers {
            updates,
            users,
            chats,
        })
    }

    // ========================================
    // Getting and applying channel difference.
    // ========================================

    fn get_channel_difference(&self) -> Option<Vec<u8>> {
        match self.0.get_channel_difference() {
            None => None,
            Some(request) => Some(request.to_bytes()), // tl::functions::updates::GetChannelDifference
        }
    }

    fn apply_channel_difference(&mut self, difference: PyBuffer<u8>) -> PyResult<PyUpdateAndPeers> {
        let difference = Python::attach(|py| difference.to_vec(py))?;
        let difference = tl::enums::updates::ChannelDifference::from_bytes(&difference)
            .map_err(|e| DeserializeError::new_err(e.to_string()))?;
        let (updates, users, chats) = self.0.apply_channel_difference(difference);
        Ok(PyUpdateAndPeers {
            updates,
            users,
            chats,
        })
    }

    fn end_channel_difference(&mut self, reason: PyPrematureEndReason) {
        self.0.end_channel_difference(reason.into());
    }
}
