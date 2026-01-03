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
fn test_comparison_operators() {
    // Greater than
    let result = execute_script("vl x = 5 > 3; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    let result = execute_script("vl x = 3 > 5; res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));

    // Less than
    let result = execute_script("vl x = 3 < 5; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    let result = execute_script("vl x = 5 < 3; res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));

    // Greater than or equal
    let result = execute_script("vl x = 5 >= 5; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    let result = execute_script("vl x = 5 >= 3; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    let result = execute_script("vl x = 3 >= 5; res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));

    // Less than or equal
    let result = execute_script("vl x = 3 <= 5; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    let result = execute_script("vl x = 5 <= 5; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    let result = execute_script("vl x = 5 <= 3; res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));
}

#[test]
fn test_modulo_operator() {
    let result = execute_script("vl x = 10 % 3; res x;").unwrap();
    assert_eq!(result, Value::Number(1.0));

    let result = execute_script("vl x = 7 % 2; res x;").unwrap();
    assert_eq!(result, Value::Number(1.0));

    let result = execute_script("vl x = 8 % 4; res x;").unwrap();
    assert_eq!(result, Value::Number(0.0));
}

#[test]
fn test_unary_operators() {
    // Negation
    let result = execute_script("vl x = -5; res x;").unwrap();
    assert_eq!(result, Value::Number(-5.0));

    let result = execute_script("vl x = -(-3); res x;").unwrap();
    assert_eq!(result, Value::Number(3.0));

    // Logical NOT
    let result = execute_script("vl x = !true; res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));

    let result = execute_script("vl x = !false; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    let result = execute_script("vl x = !0; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    let result = execute_script("vl x = !5; res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));

    // Unary plus
    let result = execute_script("vl x = +5; res x;").unwrap();
    assert_eq!(result, Value::Number(5.0));
}

#[test]
fn test_logical_operators_and() {
    // AND operator - both true
    let result = execute_script("vl x = true && true; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    // AND operator - first false (short-circuit)
    let result = execute_script("vl x = false && true; res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));

    // AND operator - second false
    let result = execute_script("vl x = true && false; res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));

    // AND operator - both false
    let result = execute_script("vl x = false && false; res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));

    // AND with numbers (truthy/falsy)
    let result = execute_script("vl x = 5 && 3; res x;").unwrap();
    assert_eq!(result, Value::Number(3.0));

    let result = execute_script("vl x = 0 && 5; res x;").unwrap();
    assert_eq!(result, Value::Number(0.0));
}

#[test]
fn test_logical_operators_or() {
    // OR operator - both true
    let result = execute_script("vl x = true || true; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    // OR operator - first true (short-circuit)
    let result = execute_script("vl x = true || false; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    // OR operator - second true
    let result = execute_script("vl x = false || true; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    // OR operator - both false
    let result = execute_script("vl x = false || false; res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));

    // OR with numbers (truthy/falsy)
    let result = execute_script("vl x = 0 || 5; res x;").unwrap();
    assert_eq!(result, Value::Number(5.0));

    let result = execute_script("vl x = 3 || 5; res x;").unwrap();
    assert_eq!(result, Value::Number(3.0));
}

#[test]
fn test_typeof_operator() {
    let result = execute_script("vl x = typeof 42; res x;").unwrap();
    assert_eq!(result, Value::String("number".to_string()));

    let result = execute_script("vl x = typeof \"hello\"; res x;").unwrap();
    assert_eq!(result, Value::String("string".to_string()));

    let result = execute_script("vl x = typeof true; res x;").unwrap();
    assert_eq!(result, Value::String("boolean".to_string()));

    let result = execute_script("vl x = typeof [1, 2, 3]; res x;").unwrap();
    assert_eq!(result, Value::String("array".to_string()));

    let result = execute_script("vl x = typeof {a: 1}; res x;").unwrap();
    assert_eq!(result, Value::String("object".to_string()));
}

#[test]
fn test_undefined_value() {
    // Undefined literal (once parser supports it)
    // For now, test object property access that returns undefined
    let result = execute_script("vl obj = {a: 1}; vl x = obj.nonexistent; res x;").unwrap();
    assert_eq!(result, Value::Undefined);
}

#[test]
fn test_grouped_expressions() {
    // Parentheses for grouping
    let result = execute_script("vl x = (2 + 3) * 4; res x;").unwrap();
    assert_eq!(result, Value::Number(20.0));

    let result = execute_script("vl x = 2 + (3 * 4); res x;").unwrap();
    assert_eq!(result, Value::Number(14.0));
}

#[test]
fn test_complex_expressions() {
    // Multiple operators
    let result = execute_script("vl x = 10 % 3 + 5 * 2; res x;").unwrap();
    assert_eq!(result, Value::Number(11.0));

    // Comparison with arithmetic
    let result = execute_script("vl x = (5 + 3) > (2 * 4); res x;").unwrap();
    assert_eq!(result, Value::Boolean(false));

    // Logical with comparison
    let result = execute_script("vl x = (5 > 3) && (2 < 4); res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));

    // Complex logical expression
    let result = execute_script("vl x = (5 > 3) || (2 > 4) && false; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_else_clause() {
    // If with else - condition true
    let script = r#"
        vl x = 5;
        vl result = 0;
        if x > 3 {
            result = 10;
        } else {
            result = 20;
        }
        res result;
    "#;
    let result = execute_script(script).unwrap();
    assert_eq!(result, Value::Number(10.0));

    // If with else - condition false
    let script = r#"
        vl x = 2;
        vl result = 0;
        if x > 3 {
            result = 10;
        } else {
            result = 20;
        }
        res result;
    "#;
    let result = execute_script(script).unwrap();
    assert_eq!(result, Value::Number(20.0));
}

#[test]
fn test_nested_conditionals() {
    let script = r#"
        vl x = 5;
        vl y = 10;
        vl result = 0;
        
        if x > 0 {
            if y > 5 {
                result = 100;
            } else {
                result = 50;
            }
        } else {
            result = 0;
        }
        
        res result;
    "#;
    let result = execute_script(script).unwrap();
    assert_eq!(result, Value::Number(100.0));
}

#[test]
fn test_operator_precedence() {
    // Multiplication before addition
    let result = execute_script("vl x = 2 + 3 * 4; res x;").unwrap();
    assert_eq!(result, Value::Number(14.0));

    // Division before subtraction
    let result = execute_script("vl x = 10 - 8 / 2; res x;").unwrap();
    assert_eq!(result, Value::Number(6.0));

    // Comparison before logical
    let result = execute_script("vl x = 5 > 3 && 2 < 4; res x;").unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_return_propagates_out_of_if() {
    // Previously: return inside an if could be swallowed by nested checks.
    let script = r#"
        vl x = 1;
        if x == 1 {
            res 123;
        }
        res 999;
    "#;
    let result = execute_script(script).unwrap();
    assert_eq!(result, Value::Number(123.0));
}

#[test]
fn test_continue_inside_if_skips_rest_of_iteration() {
    let script = r#"
        vl sum = 0;
        loop x in [1, 2, 3] {
            if x == 2 {
                continue;
            }
            sum = sum + x;
        }
        res sum;
    "#;
    let result = execute_script(script).unwrap();
    assert_eq!(result, Value::Number(4.0));
}

#[test]
fn test_end_inside_if_breaks_loop() {
    let script = r#"
        vl sum = 0;
        loop x in [1, 2, 3] {
            if x == 2 {
                end;
            }
            sum = sum + x;
        }
        res sum;
    "#;
    let result = execute_script(script).unwrap();
    assert_eq!(result, Value::Number(1.0));
}

#[test]
fn test_continue_does_not_leak_out_of_inner_loop() {
    // If the inner loop continues on its last iteration, the continue flag
    // must not affect the outer loop body.
    let script = r#"
        vl out = 0;
        loop x in [1] {
            loop y in [1] {
                continue;
            }
            out = out + 1;
        }
        res out;
    "#;
    let result = execute_script(script).unwrap();
    assert_eq!(result, Value::Number(1.0));
}
