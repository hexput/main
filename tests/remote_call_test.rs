//! Tests for remote function call execution in the VM

use hexput::language::parse;
use hexput::runtime::{execute, Context, RpcHandler, RuntimeError, Value};
use hexput::sandbox::Limits;
use hexput::semantic::capabilities::{Capability, CapabilitySet};
use std::sync::{Arc, Mutex};

/// Mock RPC handler that tracks calls
struct MockRpcHandler {
    calls: Mutex<Vec<(String, Vec<Value>)>>,
    return_value: Value,
}

impl MockRpcHandler {
    fn new(return_value: Value) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            return_value,
        }
    }

    fn get_calls(&self) -> Vec<(String, Vec<Value>)> {
        self.calls.lock().unwrap().clone()
    }
}

impl RpcHandler for MockRpcHandler {
    fn call_remote(&self, function: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        self.calls
            .lock()
            .unwrap()
            .push((function.to_string(), args));
        Ok(self.return_value.clone())
    }

    fn call_method(
        &self,
        _object: &Value,
        _method: &str,
        _args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        Ok(Value::Undefined)
    }
}

/// Mock RPC handler that always fails
struct FailingRpcHandler;

impl RpcHandler for FailingRpcHandler {
    fn call_remote(&self, function: &str, _args: Vec<Value>) -> Result<Value, RuntimeError> {
        Err(RuntimeError::RemoteCallFailed(format!(
            "Remote function '{}' failed",
            function
        )))
    }

    fn call_method(
        &self,
        _object: &Value,
        method: &str,
        _args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        Err(RuntimeError::RemoteCallFailed(format!(
            "Remote method '{}' failed",
            method
        )))
    }
}

#[test]
fn test_remote_call_with_capability() {
    let source = r#"
        vl result = remoteAdd(10, 20);
        res result;
    "#;

    let ast = parse(source).unwrap();

    let handler = Arc::new(MockRpcHandler::new(Value::Number(30.0)));
    let mut capabilities = CapabilitySet::new();
    capabilities.grant(Capability::CallRemote("remoteAdd".to_string()));

    let mut context = Context::with_rpc_handler(Limits::default(), capabilities, handler.clone());

    let result = execute(&ast, &mut context).unwrap();

    assert_eq!(result, Value::Number(30.0));

    // Verify the remote call was made
    let calls = handler.get_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "remoteAdd");
    assert_eq!(calls[0].1.len(), 2);
    assert_eq!(calls[0].1[0], Value::Number(10.0));
    assert_eq!(calls[0].1[1], Value::Number(20.0));
}

#[test]
fn test_remote_call_without_capability_fails() {
    let source = r#"
        vl result = remoteAdd(10, 20);
        res result;
    "#;

    let ast = parse(source).unwrap();

    let handler = Arc::new(MockRpcHandler::new(Value::Number(30.0)));
    // No capabilities granted
    let capabilities = CapabilitySet::new();

    let mut context = Context::with_rpc_handler(Limits::default(), capabilities, handler);

    let result = execute(&ast, &mut context);

    assert!(result.is_err());
    match result.unwrap_err() {
        RuntimeError::PermissionDenied(msg) => {
            assert!(msg.contains("remoteAdd"));
        }
        e => panic!("Expected PermissionDenied, got {:?}", e),
    }
}

#[test]
fn test_remote_call_without_handler_fails() {
    let source = r#"
        vl result = remoteAdd(10, 20);
        res result;
    "#;

    let ast = parse(source).unwrap();

    let mut capabilities = CapabilitySet::new();
    capabilities.grant(Capability::CallRemote("remoteAdd".to_string()));

    // No RPC handler set
    let mut context = Context::with_capabilities(Limits::default(), capabilities);

    let result = execute(&ast, &mut context);

    assert!(result.is_err());
    match result.unwrap_err() {
        RuntimeError::UndefinedFunction(func) => {
            assert_eq!(func, "remoteAdd");
        }
        e => panic!("Expected UndefinedFunction, got {:?}", e),
    }
}

#[test]
fn test_local_callback_takes_precedence_over_remote() {
    let source = r#"
        cb myFunc(x) {
            res x + 100;
        }
        vl result = myFunc(5);
        res result;
    "#;

    let ast = parse(source).unwrap();

    let handler = Arc::new(MockRpcHandler::new(Value::Number(999.0)));
    let mut capabilities = CapabilitySet::new();
    capabilities.grant(Capability::CallRemote("myFunc".to_string()));

    let mut context = Context::with_rpc_handler(Limits::default(), capabilities, handler.clone());

    let result = execute(&ast, &mut context).unwrap();

    // Should use local callback, not remote
    assert_eq!(result, Value::Number(105.0));

    // No remote calls should have been made
    let calls = handler.get_calls();
    assert_eq!(calls.len(), 0);
}

#[test]
fn test_remote_call_with_string_args_and_result() {
    let source = r#"
        vl greeting = remoteGreet("Alice");
        res greeting;
    "#;

    let ast = parse(source).unwrap();

    let handler = Arc::new(MockRpcHandler::new(Value::String(
        "Hello, Alice!".to_string(),
    )));
    let mut capabilities = CapabilitySet::new();
    capabilities.grant(Capability::CallRemote("remoteGreet".to_string()));

    let mut context = Context::with_rpc_handler(Limits::default(), capabilities, handler.clone());

    let result = execute(&ast, &mut context).unwrap();

    assert_eq!(result, Value::String("Hello, Alice!".to_string()));

    let calls = handler.get_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "remoteGreet");
    assert_eq!(calls[0].1[0], Value::String("Alice".to_string()));
}

#[test]
fn test_remote_call_failure_propagates() {
    let source = r#"
        vl result = failingFunc();
        res result;
    "#;

    let ast = parse(source).unwrap();

    let handler = Arc::new(FailingRpcHandler);
    let mut capabilities = CapabilitySet::new();
    capabilities.grant(Capability::CallRemote("failingFunc".to_string()));

    let mut context = Context::with_rpc_handler(Limits::default(), capabilities, handler);

    let result = execute(&ast, &mut context);

    assert!(result.is_err());
    match result.unwrap_err() {
        RuntimeError::RemoteCallFailed(msg) => {
            assert!(msg.contains("failingFunc"));
        }
        e => panic!("Expected RemoteCallFailed, got {:?}", e),
    }
}

#[test]
fn test_multiple_remote_calls() {
    let source = r#"
        vl a = remoteAdd(1, 2);
        vl b = remoteAdd(3, 4);
        vl c = remoteAdd(a, b);
        res c;
    "#;

    let ast = parse(source).unwrap();

    let handler = Arc::new(MockRpcHandler::new(Value::Number(10.0)));
    let mut capabilities = CapabilitySet::new();
    capabilities.grant(Capability::CallRemote("remoteAdd".to_string()));

    let mut context = Context::with_rpc_handler(Limits::default(), capabilities, handler.clone());

    let result = execute(&ast, &mut context).unwrap();

    assert_eq!(result, Value::Number(10.0));

    // Three remote calls should have been made
    let calls = handler.get_calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].0, "remoteAdd");
    assert_eq!(calls[1].0, "remoteAdd");
    assert_eq!(calls[2].0, "remoteAdd");
}

#[test]
fn test_remote_call_with_complex_args() {
    let source = r#"
        vl obj = { name: "Alice", age: 30 };
        vl arr = [1, 2, 3];
        vl result = processData(obj, arr);
        res result;
    "#;

    let ast = parse(source).unwrap();

    let handler = Arc::new(MockRpcHandler::new(Value::Boolean(true)));
    let mut capabilities = CapabilitySet::new();
    capabilities.grant(Capability::CallRemote("processData".to_string()));

    let mut context = Context::with_rpc_handler(Limits::default(), capabilities, handler.clone());

    let result = execute(&ast, &mut context).unwrap();

    assert_eq!(result, Value::Boolean(true));

    let calls = handler.get_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "processData");
    assert_eq!(calls[0].1.len(), 2);

    // First arg should be an object
    match &calls[0].1[0] {
        Value::Object(_) => {}
        _ => panic!("Expected object"),
    }

    // Second arg should be an array
    match &calls[0].1[1] {
        Value::Array(arr) => {
            assert_eq!(arr.len(), 3);
        }
        _ => panic!("Expected array"),
    }
}
