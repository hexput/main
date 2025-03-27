pub mod ast_structs;
pub mod lexer;
pub mod parser;
pub mod optimizer;
pub mod feature_flags;
pub mod parallel;

use serde_json::{to_string_pretty, to_string, Value};
use feature_flags::FeatureFlags;
use parser::ParseError;

pub fn process_code(code: &str, feature_flags: FeatureFlags) -> Result<ast_structs::Program, ParseError> {
    let runtime = parallel::create_runtime();
    
    let tokens = lexer::tokenize(code);
    
    let mut parser = parser::Parser::new(&tokens, feature_flags, code);
    let ast = parser.parse_program()?;
    
    let optimized_ast = optimizer::optimize_ast(ast, &runtime);
    
    Ok(optimized_ast)
}

pub fn filter_locations(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            map.remove("location");
            
            let filtered_map = map.into_iter()
                .map(|(k, v)| (k, filter_locations(v)))
                .collect();
            
            Value::Object(filtered_map)
        },
        Value::Array(arr) => {
            Value::Array(arr.into_iter().map(filter_locations).collect())
        },
        _ => value,
    }
}

pub fn to_json_string(value: &impl serde::Serialize, include_source_mapping: bool) -> Result<String, serde_json::Error> {
    if include_source_mapping {
        to_string(value)
    } else {
        let json_value = serde_json::to_value(value)?;
        let filtered = filter_locations(json_value);
        to_string(&filtered)
    }
}

pub fn to_json_string_pretty(value: &impl serde::Serialize, include_source_mapping: bool) -> Result<String, serde_json::Error> {
    if include_source_mapping {
        to_string_pretty(value)
    } else {
        let json_value = serde_json::to_value(value)?;
        let filtered = filter_locations(json_value);
        to_string_pretty(&filtered)
    }
}

pub fn format_error_as_json(error: &ParseError, minify: bool) -> String {
    let error_json = serde_json::json!({
        "error": {
            "type": "ParseError",
            "message": format!("{}", error)
        }
    });
    
    if minify {
        to_string(&error_json).unwrap_or_else(|_| String::from(r#"{"error":{"type":"ParseError","message":"JSON serialization error"}}"#))
    } else {
        to_string_pretty(&error_json).unwrap_or_else(|_| String::from(r#"{"error":{"type":"ParseError","message":"JSON serialization error"}}"#))
    }
}
