mod errors;
mod sender;

#[pyo3::pymodule]
mod _rs {
    #[pymodule_export]
    #[allow(non_upper_case_globals)]
    const __version__: &str = &env!("CARGO_PKG_VERSION");

    #[pymodule_export]
    #[allow(non_upper_case_globals)]
    const __doc__: &str = &"Python bindings for grammers-mtsender library";

    #[pymodule_export]
    use crate::sender::PySender;

    #[pymodule_export]
    use crate::errors::PyRpcError;

    #[pymodule_export]
    use crate::errors::DroppedError;

    #[pymodule_export]
    use crate::errors::DeserializeError;

    #[pymodule_export]
    use crate::errors::TransportError;
}
