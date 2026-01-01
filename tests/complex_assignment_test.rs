//! Tests for complex assignment targets (nested objects/arrays)

use hexput::language::parse;
use hexput::runtime::{execute, Context, Value};
use hexput::sandbox::Limits;

/// Helper to execute a hexput script and return the result
fn execute_script(source: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let ast = parse(source)?;
    let mut context = Context::new(Limits::default());
    let result = execute(&ast, &mut context)?;
    Ok(result)
}

#[test]
fn test_simple_array_assignment() {
    let result = execute_script(r#"
        vl arr = [1, 2, 3];
        arr[0] = 99;
        res arr[0];
    "#).unwrap();
    
    assert_eq!(result, Value::Number(99.0));
}

#[test]
fn test_simple_object_property_assignment() {
    let result = execute_script(r#"
        vl obj = { x: 10, y: 20 };
        obj.x = 42;
        res obj.x;
    "#).unwrap();
    
    assert_eq!(result, Value::Number(42.0));
}

#[test]
fn test_nested_object_property_assignment() {
    let result = execute_script(r#"
        vl obj = { inner: { value: 5 } };
        obj.inner.value = 100;
        res obj.inner.value;
    "#).unwrap();
    
    assert_eq!(result, Value::Number(100.0));
}

#[test]
fn test_nested_array_assignment() {
    let result = execute_script(r#"
        vl matrix = [[1, 2], [3, 4]];
        matrix[0][1] = 99;
        res matrix[0][1];
    "#).unwrap();
    
    assert_eq!(result, Value::Number(99.0));
}

#[test]
fn test_array_of_objects_assignment() {
    let result = execute_script(r#"
        vl arr = [{ val: 10 }, { val: 20 }];
        arr[1].val = 777;
        res arr[1].val;
    "#).unwrap();
    
    assert_eq!(result, Value::Number(777.0));
}

#[test]
fn test_object_with_array_property_assignment() {
    let result = execute_script(r#"
        vl obj = { numbers: [1, 2, 3] };
        obj.numbers[2] = 999;
        res obj.numbers[2];
    "#).unwrap();
    
    assert_eq!(result, Value::Number(999.0));
}

#[test]
fn test_deep_nested_assignment() {
    let result = execute_script(r#"
        vl data = { 
            level1: { 
                level2: { 
                    level3: 42 
                } 
            } 
        };
        data.level1.level2.level3 = 123;
        res data.level1.level2.level3;
    "#).unwrap();
    
    assert_eq!(result, Value::Number(123.0));
}

#[test]
fn test_assignment_preserves_other_fields() {
    let result = execute_script(r#"
        vl obj = { a: 1, b: 2, c: 3 };
        obj.b = 99;
        vl sum = obj.a + obj.b + obj.c;
        res sum;
    "#).unwrap();
    
    // Should be 1 + 99 + 3 = 103
    assert_eq!(result, Value::Number(103.0));
}

#[test]
fn test_assignment_preserves_other_array_elements() {
    let result = execute_script(r#"
        vl arr = [10, 20, 30, 40];
        arr[1] = 999;
        vl sum = arr[0] + arr[1] + arr[2] + arr[3];
        res sum;
    "#).unwrap();
    
    // Should be 10 + 999 + 30 + 40 = 1079
    assert_eq!(result, Value::Number(1079.0));
}

#[test]
fn test_multiple_nested_assignments() {
    let result = execute_script(r#"
        vl data = { x: { y: 1 }, z: [5, 6] };
        data.x.y = 100;
        data.z[0] = 200;
        vl sum = data.x.y + data.z[0];
        res sum;
    "#).unwrap();
    
    assert_eq!(result, Value::Number(300.0));
}

#[test]
fn test_assignment_to_loop_variable_array_element() {
    let result = execute_script(r#"
        vl arrays = [[1, 2], [3, 4], [5, 6]];
        vl sum = 0;
        loop arr in arrays {
            arr[0] = arr[0] * 10;
            sum = sum + arr[0];
        }
        res sum;
    "#).unwrap();
    
    // Should be 10 + 30 + 50 = 90
    assert_eq!(result, Value::Number(90.0));
}

#[test]
fn test_assignment_with_expression_index() {
    let result = execute_script(r#"
        vl arr = [1, 2, 3, 4, 5];
        vl idx = 2 + 1;
        arr[idx] = 999;
        res arr[3];
    "#).unwrap();
    
    assert_eq!(result, Value::Number(999.0));
}

#[test]
fn test_array_index_out_of_bounds_assignment_fails() {
    let result = execute_script(r#"
        vl arr = [1, 2, 3];
        arr[10] = 999;
        res arr[0];
    "#);
    
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("out of bounds") || err.to_string().contains("IndexOutOfBounds"));
}

#[test]
fn test_assignment_to_non_object_property_fails() {
    let result = execute_script(r#"
        vl num = 42;
        num.prop = 10;
        res num;
    "#);
    
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Type error") || err.to_string().contains("object"));
}

#[test]
fn test_assignment_to_non_array_index_fails() {
    let result = execute_script(r#"
        vl str = "hello";
        str[0] = "x";
        res str;
    "#);
    
    assert!(result.is_err());
}

#[test]
fn test_complex_assignment_in_callback() {
    let result = execute_script(r#"
        vl state = { counter: 0 };
        
        cb increment() {
            state.counter = state.counter + 1;
            res state.counter;
        }
        
        vl r1 = increment();
        vl r2 = increment();
        vl r3 = increment();
        
        res state.counter;
    "#).unwrap();
    
    assert_eq!(result, Value::Number(3.0));
}

#[test]
fn test_nested_assignment_returns_correct_value() {
    let result = execute_script(r#"
        vl obj = { inner: { value: 5 } };
        vl old = obj.inner.value;
        obj.inner.value = 20;
        vl new = obj.inner.value;
        res new - old;
    "#).unwrap();
    
    assert_eq!(result, Value::Number(15.0));
}

#[test]
fn test_assignment_to_computed_property() {
    let result = execute_script(r#"
        vl obj = { a: 10, b: 20, c: 30 };
        vl arr = [1, 2, 3];
        arr[1] = 100;
        obj.b = arr[1];
        res obj.b;
    "#).unwrap();
    
    assert_eq!(result, Value::Number(100.0));
}

#[test]
fn test_deeply_nested_mixed_assignment() {
    let result = execute_script(r#"
        vl data = {
            users: [
                { name: "Alice", scores: [90, 85] },
                { name: "Bob", scores: [75, 80] }
            ]
        };
        
        data.users[0].scores[1] = 95;
        
        res data.users[0].scores[1];
    "#).unwrap();
    
    assert_eq!(result, Value::Number(95.0));
}
