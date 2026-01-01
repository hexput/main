//! RPC layer: Protocol, dispatcher, registry

pub mod protocol;
pub mod dispatcher;
pub mod registry;
pub mod error;

pub use protocol::{Message, Request, Response};
pub use dispatcher::Dispatcher;
pub use registry::{FunctionRegistry, Registry};
pub use error::RpcError;
