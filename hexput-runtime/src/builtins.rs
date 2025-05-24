use crate::error::RuntimeError;
use hexput_ast_api::ast_structs::SourceLocation;
use serde_json::{Map, Value};
use std::future::Future;
use std::pin::Pin;

const FORBIDDEN_KEY: &str = "secret_data";
const CALLBACK_REFERENCE_HASH: &str = "__callback_ref_constant";

pub type CallbackExecutor = Box<dyn Fn(String, Vec<Value>) -> Pin<Box<dyn Future<Output = Result<Value, RuntimeError>> + Send>> + Send + Sync>;

pub fn execute_builtin_method(
    object: &Value,
    method_name: &str,
    args: &[Value],
    location: &SourceLocation,
    callback_executor: Option<&CallbackExecutor>,
) -> Result<Option<Value>, RuntimeError> {
    // Check if object contains forbidden value - if so, deny all method calls
    if contains_forbidden_value(object) {
        return Err(RuntimeError::with_location(
            "Cannot call methods on object containing restricted data. Use as reference only.".to_string(),
            location.clone(),
        ));
    }

    match object {
        Value::String(s) => execute_string_method(s, method_name, args, location),
        Value::Array(arr) => execute_array_method(arr, method_name, args, location, callback_executor),
        Value::Object(obj) => execute_object_method(obj, method_name, args, location),
        Value::Number(num) => execute_number_method(num, method_name, args, location),
        Value::Bool(b) => execute_boolean_method(b, method_name, args, location),
        Value::Null => execute_null_method(method_name, args, location),
    }
}

fn execute_string_method(
    string: &str,
    method_name: &str,
    args: &[Value],
    location: &SourceLocation,
) -> Result<Option<Value>, RuntimeError> {
    match method_name {
        "len" | "length" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("String.{} expects 0 arguments, got {}", method_name, args.len()),
                    location.clone(),
                ));
            }
            Ok(Some(Value::Number(string.len().into())))
        }
        "isEmpty" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("String.isEmpty expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            Ok(Some(Value::Bool(string.is_empty())))
        }
        "substring" => {
            if args.len() < 1 || args.len() > 2 {
                return Err(RuntimeError::with_location(
                    format!("String.substring expects 1-2 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }

            
            let start = get_index_arg(&args[0], 0, string.len(), "substring", location)?;
            
            
            let end = if args.len() > 1 {
                get_index_arg(&args[1], start, string.len(), "substring", location)?
            } else {
                string.len()
            };

            
            let chars: Vec<char> = string.chars().collect();
            if start <= end && end <= chars.len() {
                let result: String = chars[start..end].iter().collect();
                Ok(Some(Value::String(result)))
            } else {
                Ok(Some(Value::String(String::new())))
            }
        }
        "toLowerCase" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("String.toLowerCase expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            Ok(Some(Value::String(string.to_lowercase())))
        }
        "toUpperCase" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("String.toUpperCase expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            Ok(Some(Value::String(string.to_uppercase())))
        }
        "trim" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("String.trim expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            Ok(Some(Value::String(string.trim().to_string())))
        }
        "contains" | "includes" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("String.{} expects 1 argument, got {}", method_name, args.len()),
                    location.clone(),
                ));
            }
            
            match &args[0] {
                Value::String(substring) => Ok(Some(Value::Bool(string.contains(substring)))),
                _ => Err(RuntimeError::with_location(
                    format!("String.{} expects a string argument", method_name),
                    location.clone(),
                )),
            }
        }
        "startsWith" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("String.startsWith expects 1 argument, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            match &args[0] {
                Value::String(prefix) => Ok(Some(Value::Bool(string.starts_with(prefix)))),
                _ => Err(RuntimeError::with_location(
                    "String.startsWith expects a string argument".to_string(),
                    location.clone(),
                )),
            }
        }
        "endsWith" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("String.endsWith expects 1 argument, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            match &args[0] {
                Value::String(suffix) => Ok(Some(Value::Bool(string.ends_with(suffix)))),
                _ => Err(RuntimeError::with_location(
                    "String.endsWith expects a string argument".to_string(),
                    location.clone(),
                )),
            }
        }
        "indexOf" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("String.indexOf expects 1 argument, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            match &args[0] {
                Value::String(substring) => {
                    let index = string.find(substring).map_or(-1, |i| i as i64);
                    Ok(Some(Value::Number(index.into())))
                },
                _ => Err(RuntimeError::with_location(
                    "String.indexOf expects a string argument".to_string(),
                    location.clone(),
                )),
            }
        }
        "split" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("String.split expects 1 argument, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            match &args[0] {
                Value::String(delimiter) => {
                    let parts: Vec<Value> = string.split(delimiter)
                        .map(|s| Value::String(s.to_string()))
                        .collect();
                    Ok(Some(Value::Array(parts)))
                },
                _ => Err(RuntimeError::with_location(
                    "String.split expects a string argument".to_string(),
                    location.clone(),
                )),
            }
        }
        "replace" => {
            if args.len() != 2 {
                return Err(RuntimeError::with_location(
                    format!("String.replace expects 2 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            let old = match &args[0] {
                Value::String(s) => s,
                _ => return Err(RuntimeError::with_location(
                    "String.replace expects a string argument".to_string(),
                    location.clone(),
                )),
            };
            
            let new = match &args[1] {
                Value::String(s) => s,
                _ => return Err(RuntimeError::with_location(
                    "String.replace expects a string argument".to_string(),
                    location.clone(),
                )),
            };
            
            let result = string.replace(old, new);
            Ok(Some(Value::String(result)))
        }
        _ => Ok(None), 
    }
}


fn execute_array_method(
    array: &[Value],
    method_name: &str,
    args: &[Value],
    location: &SourceLocation,
    callback_executor: Option<&CallbackExecutor>,
) -> Result<Option<Value>, RuntimeError> {
    match method_name {
        "length" | "len" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Array.{} expects 0 arguments, got {}", method_name, args.len()),
                    location.clone(),
                ));
            }
            Ok(Some(Value::Number(array.len().into())))
        }
        "isEmpty" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Array.isEmpty expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            Ok(Some(Value::Bool(array.is_empty())))
        }
        "join" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Array.join expects 1 argument, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            let separator = match &args[0] {
                Value::String(s) => s,
                _ => return Err(RuntimeError::with_location(
                    "Array.join expects a string argument".to_string(),
                    location.clone(),
                )),
            };
            
            
            let items: Vec<String> = array.iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "null".to_string(),
                    Value::Array(_) => "[array]".to_string(),
                    Value::Object(_) => "[object]".to_string(),
                })
                .collect();
            
            Ok(Some(Value::String(items.join(separator))))
        }
        "first" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Array.first expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            if array.is_empty() {
                Ok(Some(Value::Null))
            } else {
                Ok(Some(array[0].clone()))
            }
        }
        "last" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Array.last expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            if array.is_empty() {
                Ok(Some(Value::Null))
            } else {
                Ok(Some(array[array.len() - 1].clone()))
            }
        }
        "includes" | "contains" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Array.{} expects 1 argument, got {}", method_name, args.len()),
                    location.clone(),
                ));
            }
            
            let target = &args[0];
            let contains = array.iter().any(|item| value_equals(item, target));
            Ok(Some(Value::Bool(contains)))
        }
        "slice" => {
            if args.len() < 1 || args.len() > 2 {
                return Err(RuntimeError::with_location(
                    format!("Array.slice expects 1-2 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            
            let start = get_index_arg(&args[0], 0, array.len(), "slice", location)?;
            
            
            let end = if args.len() > 1 {
                get_index_arg(&args[1], start, array.len(), "slice", location)?
            } else {
                array.len()
            };
            
            if start <= end && end <= array.len() {
                let result = array[start..end].to_vec();
                Ok(Some(Value::Array(result)))
            } else {
                Ok(Some(Value::Array(vec![])))
            }
        }
        "map" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Array.map expects 1 argument (callback), got {}", args.len()),
                    location.clone(),
                ));
            }
            
            // Check if the argument is a callback reference
            if let Some(callback_name) = extract_callback_name(&args[0]) {
                if callback_executor.is_some() {
                    // This will need to be handled asynchronously in the caller
                    return Ok(Some(Value::Object({
                        let mut map = Map::new();
                        map.insert("__builtin_async_op".to_string(), Value::String("map".to_string()));
                        map.insert("callback_name".to_string(), Value::String(callback_name));
                        map.insert("array".to_string(), Value::Array(array.to_vec()));
                        map
                    })));
                } else {
                    return Err(RuntimeError::with_location(
                        "Callback executor not available for Array.map".to_string(),
                        location.clone(),
                    ));
                }
            } else {
                return Err(RuntimeError::with_location(
                    "Array.map expects a callback function reference".to_string(),
                    location.clone(),
                ));
            }
        }
        "filter" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Array.filter expects 1 argument (callback), got {}", args.len()),
                    location.clone(),
                ));
            }
            
            if let Some(callback_name) = extract_callback_name(&args[0]) {
                if callback_executor.is_some() {
                    return Ok(Some(Value::Object({
                        let mut map = Map::new();
                        map.insert("__builtin_async_op".to_string(), Value::String("filter".to_string()));
                        map.insert("callback_name".to_string(), Value::String(callback_name));
                        map.insert("array".to_string(), Value::Array(array.to_vec()));
                        map
                    })));
                } else {
                    return Err(RuntimeError::with_location(
                        "Callback executor not available for Array.filter".to_string(),
                        location.clone(),
                    ));
                }
            } else {
                return Err(RuntimeError::with_location(
                    "Array.filter expects a callback function reference".to_string(),
                    location.clone(),
                ));
            }
        }
        "forEach" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Array.forEach expects 1 argument (callback), got {}", args.len()),
                    location.clone(),
                ));
            }
            
            if let Some(callback_name) = extract_callback_name(&args[0]) {
                if callback_executor.is_some() {
                    return Ok(Some(Value::Object({
                        let mut map = Map::new();
                        map.insert("__builtin_async_op".to_string(), Value::String("forEach".to_string()));
                        map.insert("callback_name".to_string(), Value::String(callback_name));
                        map.insert("array".to_string(), Value::Array(array.to_vec()));
                        map
                    })));
                } else {
                    return Err(RuntimeError::with_location(
                        "Callback executor not available for Array.forEach".to_string(),
                        location.clone(),
                    ));
                }
            } else {
                return Err(RuntimeError::with_location(
                    "Array.forEach expects a callback function reference".to_string(),
                    location.clone(),
                ));
            }
        }
        "reduce" => {
            if args.len() < 1 || args.len() > 2 {
                return Err(RuntimeError::with_location(
                    format!("Array.reduce expects 1-2 arguments (callback, initial), got {}", args.len()),
                    location.clone(),
                ));
            }
            
            if let Some(callback_name) = extract_callback_name(&args[0]) {
                if callback_executor.is_some() {
                    let initial_value = if args.len() > 1 {
                        args[1].clone()
                    } else {
                        Value::Null
                    };
                    
                    return Ok(Some(Value::Object({
                        let mut map = Map::new();
                        map.insert("__builtin_async_op".to_string(), Value::String("reduce".to_string()));
                        map.insert("callback_name".to_string(), Value::String(callback_name));
                        map.insert("array".to_string(), Value::Array(array.to_vec()));
                        map.insert("initial_value".to_string(), initial_value);
                        map.insert("has_initial".to_string(), Value::Bool(args.len() > 1));
                        map
                    })));
                } else {
                    return Err(RuntimeError::with_location(
                        "Callback executor not available for Array.reduce".to_string(),
                        location.clone(),
                    ));
                }
            } else {
                return Err(RuntimeError::with_location(
                    "Array.reduce expects a callback function reference".to_string(),
                    location.clone(),
                ));
            }
        }
        "find" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Array.find expects 1 argument (callback), got {}", args.len()),
                    location.clone(),
                ));
            }
            
            if let Some(callback_name) = extract_callback_name(&args[0]) {
                if callback_executor.is_some() {
                    return Ok(Some(Value::Object({
                        let mut map = Map::new();
                        map.insert("__builtin_async_op".to_string(), Value::String("find".to_string()));
                        map.insert("callback_name".to_string(), Value::String(callback_name));
                        map.insert("array".to_string(), Value::Array(array.to_vec()));
                        map
                    })));
                } else {
                    return Err(RuntimeError::with_location(
                        "Callback executor not available for Array.find".to_string(),
                        location.clone(),
                    ));
                }
            } else {
                return Err(RuntimeError::with_location(
                    "Array.find expects a callback function reference".to_string(),
                    location.clone(),
                ));
            }
        }
        "findIndex" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Array.findIndex expects 1 argument (callback), got {}", args.len()),
                    location.clone(),
                ));
            }
            
            if let Some(callback_name) = extract_callback_name(&args[0]) {
                if callback_executor.is_some() {
                    return Ok(Some(Value::Object({
                        let mut map = Map::new();
                        map.insert("__builtin_async_op".to_string(), Value::String("findIndex".to_string()));
                        map.insert("callback_name".to_string(), Value::String(callback_name));
                        map.insert("array".to_string(), Value::Array(array.to_vec()));
                        map
                    })));
                } else {
                    return Err(RuntimeError::with_location(
                        "Callback executor not available for Array.findIndex".to_string(),
                        location.clone(),
                    ));
                }
            } else {
                return Err(RuntimeError::with_location(
                    "Array.findIndex expects a callback function reference".to_string(),
                    location.clone(),
                ));
            }
        }
        "some" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Array.some expects 1 argument (callback), got {}", args.len()),
                    location.clone(),
                ));
            }
            
            if let Some(callback_name) = extract_callback_name(&args[0]) {
                if callback_executor.is_some() {
                    return Ok(Some(Value::Object({
                        let mut map = Map::new();
                        map.insert("__builtin_async_op".to_string(), Value::String("some".to_string()));
                        map.insert("callback_name".to_string(), Value::String(callback_name));
                        map.insert("array".to_string(), Value::Array(array.to_vec()));
                        map
                    })));
                } else {
                    return Err(RuntimeError::with_location(
                        "Callback executor not available for Array.some".to_string(),
                        location.clone(),
                    ));
                }
            } else {
                return Err(RuntimeError::with_location(
                    "Array.some expects a callback function reference".to_string(),
                    location.clone(),
                ));
            }
        }
        "every" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Array.every expects 1 argument (callback), got {}", args.len()),
                    location.clone(),
                ));
            }
            
            if let Some(callback_name) = extract_callback_name(&args[0]) {
                if callback_executor.is_some() {
                    return Ok(Some(Value::Object({
                        let mut map = Map::new();
                        map.insert("__builtin_async_op".to_string(), Value::String("every".to_string()));
                        map.insert("callback_name".to_string(), Value::String(callback_name));
                        map.insert("array".to_string(), Value::Array(array.to_vec()));
                        map
                    })));
                } else {
                    return Err(RuntimeError::with_location(
                        "Callback executor not available for Array.every".to_string(),
                        location.clone(),
                    ));
                }
            } else {
                return Err(RuntimeError::with_location(
                    "Array.every expects a callback function reference".to_string(),
                    location.clone(),
                ));
            }
        }
        _ => Ok(None), 
    }
}

fn extract_callback_name(value: &Value) -> Option<String> {
    if let Value::Object(map) = value {
        if let (Some(Value::String(type_val)), Some(Value::String(name_val))) = 
            (map.get("type"), map.get("name")) {
            if type_val == "callback_reference" {
                // Check for constant hash first
                if let Some(Value::String(hash_val)) = map.get("hash") {
                    if hash_val == CALLBACK_REFERENCE_HASH {
                        return Some(name_val.clone());
                    }
                }
                // If hash doesn't match, it might be an old format or invalid
                // Still return the name for backward compatibility but this should be validated
                return Some(name_val.clone());
            }
        }
    }
    None
}

fn contains_forbidden_value(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            map.contains_key(FORBIDDEN_KEY) || map.values().any(contains_forbidden_value)
        }
        Value::Array(arr) => {
            arr.iter().any(contains_forbidden_value)
        }
        _ => false,
    }
}

fn execute_object_method(
    object: &Map<String, Value>,
    method_name: &str,
    args: &[Value],
    location: &SourceLocation,
) -> Result<Option<Value>, RuntimeError> {
    match method_name {
        "keys" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Object.keys expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }

            let keys: Vec<Value> = object.keys()
                .filter(|&k| k != FORBIDDEN_KEY) // Filter out the forbidden key
                .map(|k| Value::String(k.clone()))
                .collect();

            Ok(Some(Value::Array(keys)))
        }
        "values" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Object.values expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }

            let values: Vec<Value> = object.iter()
                .filter(|(k, _)| *k != FORBIDDEN_KEY) // Filter out the forbidden key's value
                .map(|(_, v)| v.clone())
                .collect();
            Ok(Some(Value::Array(values)))
        }
        "isEmpty" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Object.isEmpty expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            // Consider the object empty if it only contains the forbidden key
            let is_effectively_empty = object.iter().all(|(k, _)| k == FORBIDDEN_KEY);
            Ok(Some(Value::Bool(is_effectively_empty)))
        }
        "has" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Object.{} expects 1 argument, got {}", method_name, args.len()),
                    location.clone(),
                ));
            }

            match &args[0] {
                Value::String(key) => {
                    if key == FORBIDDEN_KEY {
                        Ok(Some(Value::Bool(false))) // Never report forbidden key as present
                    } else {
                        Ok(Some(Value::Bool(object.contains_key(key))))
                    }
                },
                _ => Err(RuntimeError::with_location(
                    format!("Object.{} expects a string argument", method_name),
                    location.clone(),
                )),
            }
        }
        "entries" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Object.entries expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }

            let entries: Vec<Value> = object.iter()
                .filter(|(k, _)| *k != FORBIDDEN_KEY) // Filter out the forbidden entry
                .map(|(k, v)| {
                    Value::Array(vec![
                        Value::String(k.clone()),
                        v.clone()
                    ])
                })
                .collect();

            Ok(Some(Value::Array(entries)))
        }
        _ => Ok(None), 
    }
}

fn execute_number_method(
    number: &serde_json::Number,
    method_name: &str,
    args: &[Value],
    location: &SourceLocation,
) -> Result<Option<Value>, RuntimeError> {
    match method_name {
        "toString" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Number.toString expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            Ok(Some(Value::String(number.to_string())))
        }
        "toFixed" => {
            if args.len() != 1 {
                return Err(RuntimeError::with_location(
                    format!("Number.toFixed expects 1 argument, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            let digits = match &args[0] {
                Value::Number(n) => {
                    if let Some(d) = n.as_u64() {
                        d as usize
                    } else {
                        return Err(RuntimeError::with_location(
                            "Number.toFixed expects a non-negative integer argument".to_string(),
                            location.clone(),
                        ));
                    }
                },
                _ => return Err(RuntimeError::with_location(
                    "Number.toFixed expects a number argument".to_string(),
                    location.clone(),
                )),
            };
            
            if let Some(n) = number.as_f64() {
                let formatted = format!("{:.*}", digits, n);
                Ok(Some(Value::String(formatted)))
            } else {
                Ok(Some(Value::String(number.to_string())))
            }
        }
        "isInteger" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Number.isInteger expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            Ok(Some(Value::Bool(number.is_i64() || number.is_u64())))
        }
        "abs" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Number.abs expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            if let Some(n) = number.as_f64() {
                Ok(Some(Value::Number(serde_json::Number::from_f64(n.abs()).unwrap_or_else(|| serde_json::Number::from(0)))))
            } else if let Some(n) = number.as_i64() {
                Ok(Some(Value::Number(n.abs().into())))
            } else {
                
                Ok(Some(Value::Number(number.clone())))
            }
        }
        _ => Ok(None), 
    }
}


fn execute_boolean_method(
    boolean: &bool,
    method_name: &str,
    args: &[Value],
    location: &SourceLocation,
) -> Result<Option<Value>, RuntimeError> {
    match method_name {
        "toString" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("Boolean.toString expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            Ok(Some(Value::String(boolean.to_string())))
        }
        _ => Ok(None), 
    }
}


fn execute_null_method(
    method_name: &str,
    args: &[Value],
    location: &SourceLocation,
) -> Result<Option<Value>, RuntimeError> {
    match method_name {
        "toString" => {
            if !args.is_empty() {
                return Err(RuntimeError::with_location(
                    format!("null.toString expects 0 arguments, got {}", args.len()),
                    location.clone(),
                ));
            }
            
            Ok(Some(Value::String("null".to_string())))
        }
        _ => Ok(None), 
    }
}

fn value_equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a_val), Value::Bool(b_val)) => a_val == b_val,
        (Value::Number(a_val), Value::Number(b_val)) => {
            
            if let (Some(a_f64), Some(b_f64)) = (a_val.as_f64(), b_val.as_f64()) {
                (a_f64 - b_f64).abs() < f64::EPSILON
            } else {
                a_val.to_string() == b_val.to_string()
            }
        },
        (Value::String(a_val), Value::String(b_val)) => a_val == b_val,
        _ => false, 
    }
}

fn get_index_arg(
    arg: &Value, 
    min: usize, 
    max: usize, 
    method_name: &str, 
    location: &SourceLocation
) -> Result<usize, RuntimeError> {
    match arg {
        Value::Number(n) => {
            if let Some(idx) = n.as_u64() {
                let idx = idx as usize;
                if idx >= min && idx <= max {
                    Ok(idx)
                } else {
                    Err(RuntimeError::with_location(
                        format!("Index out of bounds in {} method", method_name),
                        location.clone(),
                    ))
                }
            } else if let Some(idx) = n.as_i64() {
                if idx < 0 {
                    Err(RuntimeError::with_location(
                        format!("Negative index not allowed in {} method", method_name),
                        location.clone(),
                    ))
                } else {
                    let idx = idx as usize;
                    if idx >= min && idx <= max {
                        Ok(idx)
                    } else {
                        Err(RuntimeError::with_location(
                            format!("Index out of bounds in {} method", method_name),
                            location.clone(),
                        ))
                    }
                }
            } else {
                Err(RuntimeError::with_location(
                    format!("{} expects integer arguments", method_name),
                    location.clone(),
                ))
            }
        },
        _ => Err(RuntimeError::with_location(
            format!("{} expects number arguments", method_name),
            location.clone(),
        )),
    }
}
