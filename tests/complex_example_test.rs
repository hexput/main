use hexput::runtime::{Context, Value};
use hexput::sandbox::Limits;
use hexput::semantic::capabilities::CapabilitySet;

fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Undefined => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Number(n) => serde_json::Number::from_f64(*n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Array(arr) => serde_json::Value::Array(arr.iter().map(value_to_json).collect()),
        Value::Object(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                obj.insert(k.clone(), value_to_json(v));
            }
            serde_json::Value::Object(obj)
        }
        Value::MethodObject {
            object_id: _,
            fields,
        } => {
            let mut obj = serde_json::Map::new();
            for (k, v) in fields {
                obj.insert(k.clone(), value_to_json(v));
            }
            serde_json::Value::Object(obj)
        }
        Value::Callback { .. } => serde_json::Value::String("<callback>".to_string()),
    }
}

#[test]
fn test_05_complex_example() {
    // Read the 05_complex.hxp file (fixed version that works with hexput limitations)
    let source =
        std::fs::read_to_string("examples/05_complex.hxp").expect("Failed to read 05_complex.hxp");

    // Parse the script
    let ast = hexput::language::parse(&source).expect("Failed to parse script");

    // Create execution context
    let mut context = Context::with_capabilities(Limits::default(), CapabilitySet::new());

    // Execute the script
    let result = hexput::runtime::execute(&ast, &mut context).expect("Execution failed");

    println!("Execution successful!");
    let json_value = value_to_json(&result);
    println!(
        "Result: {}",
        serde_json::to_string_pretty(&json_value).unwrap()
    );

    // Validate the output structure
    let obj = json_value.as_object().expect("Result should be an object");

    // Check status field
    assert_eq!(
        obj.get("status").and_then(|v| v.as_str()),
        Some("complete"),
        "status should be 'complete'"
    );

    // Check results field exists (object with format names as keys)
    let results = obj.get("results").expect("results field should exist");
    assert!(results.is_object(), "results should be an object");
    let results_obj = results.as_object().unwrap();

    // Should have processed 2 items (json and csv, xml was skipped)
    assert_eq!(
        results_obj.len(),
        2,
        "Should have processed 2 items (json, csv)"
    );

    // Check json result
    let json_result = results_obj.get("json").expect("json result should exist");
    let json_obj = json_result
        .as_object()
        .expect("JSON result should be object");
    assert!(json_obj.contains_key("value"), "Should have value field");
    assert!(json_obj.contains_key("score"), "Should have score field");
    assert!(
        json_obj.contains_key("metadata"),
        "JSON should have metadata"
    );
    assert_eq!(
        json_obj.get("score").and_then(|v| v.as_f64()),
        Some(20.0),
        "JSON score should be 20"
    );

    // Check csv result
    let csv_result = results_obj.get("csv").expect("csv result should exist");
    let csv_obj = csv_result.as_object().expect("CSV result should be object");
    assert!(csv_obj.contains_key("value"), "CSV should have value field");
    assert!(csv_obj.contains_key("score"), "CSV should have score field");
    assert_eq!(
        csv_obj.get("score").and_then(|v| v.as_f64()),
        Some(15.0),
        "CSV score should be 15"
    );

    // Check summary
    let summary = obj.get("summary").expect("summary should exist");
    let summary_obj = summary.as_object().expect("summary should be object");
    assert_eq!(
        summary_obj.get("processed").and_then(|v| v.as_f64()),
        Some(2.0),
        "Should have processed 2 items"
    );
    assert_eq!(
        summary_obj.get("success").and_then(|v| v.as_bool()),
        Some(true),
        "success should be true"
    );

    // Check config
    let config = obj.get("config").expect("config should exist");
    let config_obj = config.as_object().expect("config should be object");
    assert_eq!(
        config_obj.get("name").and_then(|v| v.as_str()),
        Some("DataProcessor")
    );
    assert_eq!(
        config_obj.get("version").and_then(|v| v.as_f64()),
        Some(1.5)
    );
}

#[test]
fn test_05_complex_with_bytecode() {
    // Read the 05_complex.hxp file
    let source =
        std::fs::read_to_string("examples/05_complex.hxp").expect("Failed to read 05_complex.hxp");

    // Step 1: Compile to bytecode
    let ast = hexput::language::parse(&source).expect("Failed to parse script");

    let bincode_data = bincode::serialize(&ast).expect("Failed to serialize AST");

    let bytecode =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bincode_data);

    // Step 2: Deserialize and execute
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &bytecode)
        .expect("Failed to decode bytecode");

    let deserialized_ast: hexput::language::ast::Ast =
        bincode::deserialize(&decoded).expect("Failed to deserialize AST");

    // Create execution context
    let mut context = Context::with_capabilities(Limits::default(), CapabilitySet::new());

    // Execute the deserialized AST
    let result = hexput::runtime::execute(&deserialized_ast, &mut context)
        .expect("Bytecode execution failed");

    println!("Bytecode execution successful!");

    // Validate the same structure as the direct execution test
    let json_value = value_to_json(&result);
    let obj = json_value.as_object().expect("Result should be an object");
    assert_eq!(obj.get("status").and_then(|v| v.as_str()), Some("complete"));

    let results = obj.get("results").expect("results field should exist");
    assert!(results.is_object(), "results should be an object");
    assert_eq!(results.as_object().unwrap().len(), 2);
}
