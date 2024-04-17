use thiserror::Error;

#[derive(Debug, Error)]
pub enum TpLinkHs110Error {
    /// Encrypted response length is too short.
    #[error("encrypted response is too short (length: {0})")]
    ShortEncryptedResponse(usize),

    /// Wrapper for
    /// [`std::io::Error`](https://doc.rust-lang.org/std/io/struct.Error.html)
    #[error("IO: {0}")]
    IO(#[from] std::io::Error),

    /// Mismatch between actual response payload length and payload length specified in the response
    /// header.
    #[error(
        "encrypted response payload length ({payload_len_actual}) differs from payload length \
        specified in the header ({payload_len_from_header})"
    )]
    EncryptedPayloadLengthMismatch {
        payload_len_actual: usize,
        payload_len_from_header: u32,
    },

    /// Wrapper for
    /// [`std::array::TryFromSliceError`](https://doc.rust-lang.org/std/array/struct.TryFromSliceError.html)
    #[error("failed to construct array from slice: {0}")]
    TryFromSliceError(#[from] std::array::TryFromSliceError),

    /// Wrapper for
    /// [`std::net::AddrParseError`](https://doc.rust-lang.org/std/net/struct.AddrParseError.html)
    #[error("failed to parse an IP address: {0}")]
    AddrParse(#[from] std::net::AddrParseError),

    /// Wrapper for
    /// [`serde_json::Error`](https://docs.rs/serde_json/latest/serde_json/struct.Error.html)
    #[error("serde json: {0}")]
    SerdeJson(#[from] serde_json::Error),

    /// Given key is not available in the response.
    #[error("key {key:?} is not available in the response: {response:#?}")]
    KeyIsNotAvailable {
        response: serde_json::Value,
        key: &'static str,
    },

    /// JSON value represented in unexpected form.
    #[error("JSON value represented in unexpected form")]
    UnexpectedValueRepresentation,

    /// Smartplug reported the command has failed.
    #[error("smartplug reported the command has failed (err_code = {0})")]
    SmartplugErrCode(i64),

    /// Smartplug network port is not provided (default value is missing?).
    #[error("smartplug network port is not provided")]
    PortIsNotProvided,

    /// Smartplug host address is not provided.
    #[error("smartplug host address is not provided")]
    HostIsNotProvided,
}
