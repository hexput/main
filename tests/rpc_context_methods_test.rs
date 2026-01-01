use hexput::rpc::{Dispatcher, Message, Request, Registry};

#[test]
fn test_dispatch_function_in_default_context() {
    let mut registry = Registry::new();
    registry.register_function(None, "hello", |_args| Ok(serde_json::json!("world")));
    registry.allow_function("default", "hello");

    let dispatcher = Dispatcher::with_registry(registry);

    let msg = Message::Request(Request::new(
        "req-1".to_string(),
        "1".to_string(),
        "hello".to_string(),
        vec![],
    ));

    let out = dispatcher.dispatch(msg).unwrap();
    match out {
        Message::Response(resp) => match resp.result {
            hexput::rpc::protocol::ResponseResult::Success { value } => {
                assert_eq!(value, serde_json::json!("world"));
            }
            _ => panic!("expected success"),
        },
        _ => panic!("expected response"),
    }
}

#[test]
fn test_dispatch_function_in_named_context() {
    let mut registry = Registry::new();
    registry.register_function(Some("ctx"), "hello", |_args| Ok(serde_json::json!(123)));
    registry.allow_function("ctx", "hello");

    let dispatcher = Dispatcher::with_registry(registry);

    let msg = Message::Request(Request::in_context(
        "req-2".to_string(),
        "1".to_string(),
        "ctx".to_string(),
        "hello".to_string(),
        vec![],
    ));

    let out = dispatcher.dispatch(msg).unwrap();
    match out {
        Message::Response(resp) => match resp.result {
            hexput::rpc::protocol::ResponseResult::Success { value } => {
                assert_eq!(value, serde_json::json!(123));
            }
            _ => panic!("expected success"),
        },
        _ => panic!("expected response"),
    }
}

#[test]
fn test_dispatch_method_in_context() {
    let mut registry = Registry::new();
    registry.register_method(Some("ctx"), "obj-1", "method_name", |_args| {
        Ok(serde_json::json!({"ok": true}))
    });
    registry.allow_method("ctx", "obj-1", "method_name");

    let dispatcher = Dispatcher::with_registry(registry);

    let msg = Message::Request(Request::method_call(
        "req-3".to_string(),
        "1".to_string(),
        "ctx".to_string(),
        "obj-1".to_string(),
        "method_name".to_string(),
        vec![serde_json::json!("arg")],
    ));

    let out = dispatcher.dispatch(msg).unwrap();
    match out {
        Message::Response(resp) => match resp.result {
            hexput::rpc::protocol::ResponseResult::Success { value } => {
                assert_eq!(value, serde_json::json!({"ok": true}));
            }
            _ => panic!("expected success"),
        },
        _ => panic!("expected response"),
    }
}

#[test]
fn test_dispatch_method_missing_returns_error_response() {
    let registry = Registry::new();
    let dispatcher = Dispatcher::with_registry(registry);

    let msg = Message::Request(Request::method_call(
        "req-4".to_string(),
        "1".to_string(),
        "ctx".to_string(),
        "obj-1".to_string(),
        "missing".to_string(),
        vec![],
    ));

    let out = dispatcher.dispatch(msg).unwrap();
    match out {
        Message::Response(resp) => match resp.result {
            hexput::rpc::protocol::ResponseResult::Error { message } => {
                assert!(
                    message.contains("not found") || message.contains("No methods registered"),
                    "unexpected error message: {message}"
                );
            }
            _ => panic!("expected error"),
        },
        _ => panic!("expected response"),
    }
}
