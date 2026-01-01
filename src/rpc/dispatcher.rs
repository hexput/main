//! RPC dispatcher
//!
//! Routes incoming RPC requests to the appropriate handlers.

use super::protocol::{Message, Request, Response};
use super::registry::{FunctionRegistry, Registry};
use super::error::RpcError;

pub struct Dispatcher {
    registry: Registry,
}

impl Dispatcher {
    pub fn new(registry: FunctionRegistry) -> Self {
        Self {
            registry: Registry::from_default_functions(registry),
        }
    }

    pub fn with_registry(registry: Registry) -> Self {
        Self { registry }
    }

    pub fn dispatch(&self, message: Message) -> Result<Message, RpcError> {
        match message {
            Message::Request(request) => {
                let response = self.handle_request(request)?;
                Ok(Message::Response(response))
            }
            Message::Response(_) => {
                Err(RpcError::General("Cannot dispatch a response".to_string()))
            }
            _ => Err(RpcError::General("Unsupported message type for dispatch".to_string())),
        }
    }

    fn handle_request(&self, request: Request) -> Result<Response, RpcError> {
        let ctx = request.context_id.as_deref();
        let request_id = request.request_id.clone();
        let id = request.id.clone();

        let result = match request.object_id.as_deref() {
            Some(object_id) => self.registry.call_method(ctx, object_id, &request.function, request.args),
            None => self.registry.call_function(ctx, &request.function, request.args),
        };

        match result {
            Ok(value) => Ok(Response::success(request_id, id, value)),
            Err(err) => Ok(Response::error(request_id, id, err)),
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}
