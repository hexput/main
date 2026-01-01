//! RPC layer: Protocol, dispatcher, registry

pub mod dispatcher;
pub mod error;
pub mod protocol;
pub mod registry;

pub use dispatcher::Dispatcher;
pub use error::RpcError;
pub use protocol::{Message, Request, Response};
pub use registry::{FunctionRegistry, Registry};
