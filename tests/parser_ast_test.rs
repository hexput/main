use hexput::language::parser;
use hexput::language::ast::{Statement, Expression, BinaryOp, AssignTarget};

/// Helper to parse hexput source code
fn parse(source: &str) -> Result<hexput::language::ast::Ast, Box<dyn std::error::Error>> {
    let ast = parser::parse(source)?;
    Ok(ast)
}

#[test]
fn test_parse_binary_operators() {
    // Addition
    let ast = parse("vl x = 2 + 3;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Binary { op, .. }, .. } => {
            assert!(matches!(op, BinaryOp::Add));
        }
        _ => panic!("Expected binary addition"),
    }

    // Subtraction
    let ast = parse("vl x = 5 - 2;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Binary { op, .. }, .. } => {
            assert!(matches!(op, BinaryOp::Sub));
        }
        _ => panic!("Expected binary subtraction"),
    }

    // Multiplication
    let ast = parse("vl x = 3 * 4;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Binary { op, .. }, .. } => {
            assert!(matches!(op, BinaryOp::Mul));
        }
        _ => panic!("Expected binary multiplication"),
    }

    // Division
    let ast = parse("vl x = 8 / 2;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Binary { op, .. }, .. } => {
            assert!(matches!(op, BinaryOp::Div));
        }
        _ => panic!("Expected binary division"),
    }

    // Equality
    let ast = parse("vl x = 5 == 5;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Binary { op, .. }, .. } => {
            assert!(matches!(op, BinaryOp::Eq));
        }
        _ => panic!("Expected equality"),
    }

    // Not equal
    let ast = parse("vl x = 5 != 3;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Binary { op, .. }, .. } => {
            assert!(matches!(op, BinaryOp::NotEq));
        }
        _ => panic!("Expected not equal"),
    }
}

#[test]
fn test_parse_operator_precedence() {
    // Multiplication before addition: 2 + 3 * 4
    let ast = parse("vl x = 2 + 3 * 4;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { 
            value: Expression::Binary { 
                op: BinaryOp::Add, 
                left, 
                right 
            }, 
            .. 
        } => {
            // Left should be 2
            assert!(matches!(**left, Expression::Number(2.0)));
            // Right should be 3 * 4
            match &**right {
                Expression::Binary { op: BinaryOp::Mul, left: l2, right: r2 } => {
                    assert!(matches!(**l2, Expression::Number(3.0)));
                    assert!(matches!(**r2, Expression::Number(4.0)));
                }
                _ => panic!("Expected multiplication on right"),
            }
        }
        _ => panic!("Expected addition at top level"),
    }

    // Division before subtraction: 10 - 8 / 2
    let ast = parse("vl x = 10 - 8 / 2;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { 
            value: Expression::Binary { 
                op: BinaryOp::Sub, 
                left, 
                right 
            }, 
            .. 
        } => {
            assert!(matches!(**left, Expression::Number(10.0)));
            match &**right {
                Expression::Binary { op: BinaryOp::Div, .. } => {},
                _ => panic!("Expected division on right"),
            }
        }
        _ => panic!("Expected subtraction at top level"),
    }
}

#[test]
fn test_parse_array_literal() {
    let ast = parse("vl arr = [1, 2, 3];").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Array(elements), .. } => {
            assert_eq!(elements.len(), 3);
            assert!(matches!(elements[0], Expression::Number(1.0)));
            assert!(matches!(elements[1], Expression::Number(2.0)));
            assert!(matches!(elements[2], Expression::Number(3.0)));
        }
        _ => panic!("Expected array literal"),
    }

    // Empty array
    let ast = parse("vl arr = [];").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Array(elements), .. } => {
            assert_eq!(elements.len(), 0);
        }
        _ => panic!("Expected empty array"),
    }

    // Nested array
    let ast = parse("vl arr = [[1, 2], [3, 4]];").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Array(elements), .. } => {
            assert_eq!(elements.len(), 2);
            assert!(matches!(elements[0], Expression::Array(_)));
            assert!(matches!(elements[1], Expression::Array(_)));
        }
        _ => panic!("Expected nested array"),
    }
}

#[test]
fn test_parse_object_literal() {
    let ast = parse("vl obj = {a: 1, b: 2};").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Object(props), .. } => {
            assert_eq!(props.len(), 2);
            assert!(props.contains_key("a"));
            assert!(props.contains_key("b"));
            assert!(matches!(props.get("a").unwrap(), Expression::Number(1.0)));
            assert!(matches!(props.get("b").unwrap(), Expression::Number(2.0)));
        }
        _ => panic!("Expected object literal"),
    }

    // Empty object
    let ast = parse("vl obj = {};").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Object(props), .. } => {
            assert_eq!(props.len(), 0);
        }
        _ => panic!("Expected empty object"),
    }

    // Nested object
    let ast = parse("vl obj = {a: {b: 1}};").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Object(props), .. } => {
            assert_eq!(props.len(), 1);
            match props.get("a").unwrap() {
                Expression::Object(nested) => {
                    assert_eq!(nested.len(), 1);
                    assert!(nested.contains_key("b"));
                }
                _ => panic!("Expected nested object"),
            }
        }
        _ => panic!("Expected object literal"),
    }
}

#[test]
fn test_parse_property_access() {
    let ast = parse("vl x = obj.prop;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Property { object, property }, .. } => {
            assert!(matches!(**object, Expression::Identifier(_)));
            assert_eq!(property, "prop");
        }
        _ => panic!("Expected property access"),
    }

    // Chained property access
    let ast = parse("vl x = obj.a.b;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Property { object, property }, .. } => {
            assert_eq!(property, "b");
            match &**object {
                Expression::Property { property: p2, .. } => {
                    assert_eq!(p2, "a");
                }
                _ => panic!("Expected chained property access"),
            }
        }
        _ => panic!("Expected property access"),
    }
}

#[test]
fn test_parse_index_access() {
    let ast = parse("vl x = arr[0];").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Index { object, index }, .. } => {
            assert!(matches!(**object, Expression::Identifier(_)));
            assert!(matches!(**index, Expression::Number(0.0)));
        }
        _ => panic!("Expected index access"),
    }

    // String index
    let ast = parse("vl x = obj[\"key\"];").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Index { index, .. }, .. } => {
            assert!(matches!(**index, Expression::String(_)));
        }
        _ => panic!("Expected index access with string"),
    }

    // Computed index
    let ast = parse("vl x = arr[i + 1];").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Index { index, .. }, .. } => {
            assert!(matches!(**index, Expression::Binary { .. }));
        }
        _ => panic!("Expected index access with expression"),
    }
}

#[test]
fn test_parse_function_call() {
    let ast = parse("vl x = foo();").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Call { callee, args }, .. } => {
            assert_eq!(callee, "foo");
            assert_eq!(args.len(), 0);
        }
        _ => panic!("Expected function call"),
    }

    // With arguments
    let ast = parse("vl x = add(1, 2);").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Call { callee, args }, .. } => {
            assert_eq!(callee, "add");
            assert_eq!(args.len(), 2);
            assert!(matches!(args[0], Expression::Number(1.0)));
            assert!(matches!(args[1], Expression::Number(2.0)));
        }
        _ => panic!("Expected function call with args"),
    }

    // Nested calls
    let ast = parse("vl x = outer(inner(5));").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Call { callee, args }, .. } => {
            assert_eq!(callee, "outer");
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expression::Call { callee: inner, args: inner_args } => {
                    assert_eq!(inner, "inner");
                    assert_eq!(inner_args.len(), 1);
                }
                _ => panic!("Expected nested call"),
            }
        }
        _ => panic!("Expected function call"),
    }
}

#[test]
fn test_parse_keysof() {
    let ast = parse("vl keys = keysof obj;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::KeysOf(expr), .. } => {
            assert!(matches!(**expr, Expression::Identifier(_)));
        }
        _ => panic!("Expected keysof expression"),
    }

    // With object literal
    let ast = parse("vl keys = keysof {a: 1, b: 2};").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::KeysOf(expr), .. } => {
            assert!(matches!(**expr, Expression::Object(_)));
        }
        _ => panic!("Expected keysof with object literal"),
    }
}

#[test]
fn test_parse_assignment() {
    // Simple variable assignment
    let ast = parse("x = 10;").unwrap();
    match &ast.statements[0] {
        Statement::Assignment { target, value, .. } => {
            match target {
                AssignTarget::Identifier(name) => assert_eq!(name, "x"),
                _ => panic!("Expected identifier target"),
            }
            assert!(matches!(value, Expression::Number(10.0)));
        }
        _ => panic!("Expected assignment"),
    }

    // Property assignment
    let ast = parse("obj.prop = 5;").unwrap();
    match &ast.statements[0] {
        Statement::Assignment { target, .. } => {
            match target {
                AssignTarget::Property { object, property } => {
                    assert!(object.is_identifier());
                    assert_eq!(property, "prop");
                }
                _ => panic!("Expected property target"),
            }
        }
        _ => panic!("Expected property assignment"),
    }

    // Index assignment
    let ast = parse("arr[0] = 100;").unwrap();
    match &ast.statements[0] {
        Statement::Assignment { target, .. } => {
            match target {
                AssignTarget::Index { object, index } => {
                    assert!(object.is_identifier());
                    assert!(matches!(**index, Expression::Number(0.0)));
                }
                _ => panic!("Expected index target"),
            }
        }
        _ => panic!("Expected index assignment"),
    }
}

#[test]
fn test_parse_loop() {
    let ast = parse("loop item in items { vl x = item; }").unwrap();
    match &ast.statements[0] {
        Statement::Loop { var, iterable, body, .. } => {
            assert_eq!(var, "item");
            assert!(matches!(iterable, Expression::Identifier(_)));
            assert_eq!(body.len(), 1);
        }
        _ => panic!("Expected loop statement"),
    }

    // Loop with array literal
    let ast = parse("loop x in [1, 2, 3] { res x; }").unwrap();
    match &ast.statements[0] {
        Statement::Loop { iterable, body, .. } => {
            assert!(matches!(iterable, Expression::Array(_)));
            assert_eq!(body.len(), 1);
        }
        _ => panic!("Expected loop with array"),
    }
}

#[test]
fn test_parse_if_statement() {
    // Simple if with equality
    let ast = parse("if x == 5 { vl y = 10; }").unwrap();
    match &ast.statements[0] {
        Statement::If { condition, then_body, else_body, .. } => {
            assert!(matches!(condition, Expression::Binary { .. }));
            assert_eq!(then_body.len(), 1);
            assert!(else_body.is_none());
        }
        _ => panic!("Expected if statement"),
    }

    // If with boolean condition
    let ast = parse("if true { res 1; }").unwrap();
    match &ast.statements[0] {
        Statement::If { condition, then_body, .. } => {
            assert!(matches!(condition, Expression::Boolean(true)));
            assert_eq!(then_body.len(), 1);
        }
        _ => panic!("Expected if with boolean"),
    }
}

#[test]
fn test_parse_return() {
    let ast = parse("res 42;").unwrap();
    match &ast.statements[0] {
        Statement::Return { value, .. } => {
            assert!(matches!(value, Expression::Number(42.0)));
        }
        _ => panic!("Expected return statement"),
    }

    // Return expression
    let ast = parse("res x + y;").unwrap();
    match &ast.statements[0] {
        Statement::Return { value, .. } => {
            assert!(matches!(value, Expression::Binary { .. }));
        }
        _ => panic!("Expected return with expression"),
    }
}

#[test]
fn test_parse_continue_and_end() {
    let ast = parse("continue;").unwrap();
    assert!(matches!(ast.statements[0], Statement::Continue { .. }));

    let ast = parse("end;").unwrap();
    assert!(matches!(ast.statements[0], Statement::End { .. }));
}

#[test]
fn test_parse_callback_declaration() {
    // No parameters
    let ast = parse("cb foo() { res 1; }").unwrap();
    match &ast.statements[0] {
        Statement::CallbackDecl { name, params, body, .. } => {
            assert_eq!(name, "foo");
            assert_eq!(params.len(), 0);
            assert_eq!(body.len(), 1);
        }
        _ => panic!("Expected callback with no params"),
    }

    // Multiple parameters
    let ast = parse("cb add(a, b, c) { res a + b + c; }").unwrap();
    match &ast.statements[0] {
        Statement::CallbackDecl { name, params, body, .. } => {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 3);
            assert_eq!(params[0], "a");
            assert_eq!(params[1], "b");
            assert_eq!(params[2], "c");
            assert_eq!(body.len(), 1);
        }
        _ => panic!("Expected callback with multiple params"),
    }

    // Complex body
    let ast = parse("cb process(x) { vl y = x * 2; vl z = y + 1; res z; }").unwrap();
    match &ast.statements[0] {
        Statement::CallbackDecl { body, .. } => {
            assert_eq!(body.len(), 3);
        }
        _ => panic!("Expected callback with complex body"),
    }
}

#[test]
fn test_parse_multiple_statements() {
    let source = r#"
        vl x = 5;
        vl y = 10;
        vl sum = x + y;
        res sum;
    "#;
    let ast = parse(source).unwrap();
    assert_eq!(ast.statements.len(), 4);
}

#[test]
fn test_parse_complex_nested_structure() {
    let source = "vl config = {name: \"app\", version: 1.0, settings: {enabled: true, count: 5}}; vl items = [1, 2, 3, 4, 5]; loop item in items { if item == 3 { res item; } }";
    let ast = parse(source).unwrap();
    assert_eq!(ast.statements.len(), 3);
    
    // First statement is object with nested object
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Object(props), .. } => {
            assert_eq!(props.len(), 3);
            match props.get("settings").unwrap() {
                Expression::Object(nested) => {
                    assert_eq!(nested.len(), 2);
                }
                _ => panic!("Expected nested object in settings"),
            }
        }
        _ => panic!("Expected object declaration"),
    }
    
    // Second statement is array
    match &ast.statements[1] {
        Statement::VarDecl { value: Expression::Array(elements), .. } => {
            assert_eq!(elements.len(), 5);
        }
        _ => panic!("Expected array declaration"),
    }
    
    // Third statement is loop with if
    match &ast.statements[2] {
        Statement::Loop { body, .. } => {
            assert_eq!(body.len(), 1);
            assert!(matches!(body[0], Statement::If { .. }));
        }
        _ => panic!("Expected loop statement"),
    }
}

#[test]
fn test_parse_string_literals() {
    let ast = parse(r#"vl msg = "hello world";"#).unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::String(s), .. } => {
            assert_eq!(s, "hello world");
        }
        _ => panic!("Expected string literal"),
    }

    // Empty string
    let ast = parse(r#"vl empty = "";"#).unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::String(s), .. } => {
            assert_eq!(s, "");
        }
        _ => panic!("Expected empty string"),
    }
}

#[test]
fn test_parse_boolean_literals() {
    let ast = parse("vl t = true;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Boolean(b), .. } => {
            assert_eq!(*b, true);
        }
        _ => panic!("Expected boolean true"),
    }

    let ast = parse("vl f = false;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Boolean(b), .. } => {
            assert_eq!(*b, false);
        }
        _ => panic!("Expected boolean false"),
    }
}

#[test]
fn test_parse_number_literals() {
    // Integer
    let ast = parse("vl x = 42;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Number(n), .. } => {
            assert_eq!(*n, 42.0);
        }
        _ => panic!("Expected integer"),
    }

    // Float
    let ast = parse("vl x = 3.14;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Number(n), .. } => {
            assert_eq!(*n, 3.14);
        }
        _ => panic!("Expected float"),
    }

    // Zero
    let ast = parse("vl x = 0;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Number(n), .. } => {
            assert_eq!(*n, 0.0);
        }
        _ => panic!("Expected zero"),
    }
}

#[test]
fn test_parse_identifier() {
    let ast = parse("vl y = x;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Identifier(name), .. } => {
            assert_eq!(name, "x");
        }
        _ => panic!("Expected identifier"),
    }
}

#[test]
fn test_parse_comments() {
    let source = "// This is a comment\nvl x = 5; // Inline comment\n// Another comment\nvl y = 10;";
    let ast = parse(source).unwrap();
    assert_eq!(ast.statements.len(), 2);
}

#[test]
fn test_parse_chained_operations() {
    // Method chaining style
    let ast = parse("vl x = obj.prop.subprop;").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Property { object, property }, .. } => {
            assert_eq!(property, "subprop");
            match &**object {
                Expression::Property { property: p2, .. } => {
                    assert_eq!(p2, "prop");
                }
                _ => panic!("Expected chained property"),
            }
        }
        _ => panic!("Expected property chain"),
    }

    // Array access chain
    let ast = parse("vl x = arr[0][1];").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Index { object, index }, .. } => {
            assert!(matches!(**index, Expression::Number(1.0)));
            match &**object {
                Expression::Index { index: idx2, .. } => {
                    assert!(matches!(**idx2, Expression::Number(0.0)));
                }
                _ => panic!("Expected chained index"),
            }
        }
        _ => panic!("Expected index chain"),
    }
}

#[test]
fn test_parse_expression_statement() {
    // Function call as statement
    let ast = parse("foo();").unwrap();
    match &ast.statements[0] {
        Statement::Expression(Expression::Call { callee, .. }) => {
            assert_eq!(callee, "foo");
        }
        _ => panic!("Expected expression statement with call"),
    }
}

#[test]
fn test_parse_whitespace_insensitivity() {
    let compact = parse("vl x=5;vl y=10;").unwrap();
    let spaced = parse("vl x = 5; vl y = 10;").unwrap();
    let multiline = parse("vl x = 5;\nvl y = 10;").unwrap();
    
    assert_eq!(compact.statements.len(), 2);
    assert_eq!(spaced.statements.len(), 2);
    assert_eq!(multiline.statements.len(), 2);
}

#[test]
fn test_parse_edge_cases() {
    // Single statement
    let ast = parse("vl x = 1;").unwrap();
    assert_eq!(ast.statements.len(), 1);

    // Trailing semicolon optional
    let ast = parse("vl x = 1").unwrap();
    assert_eq!(ast.statements.len(), 1);

    // Empty object
    let ast = parse("vl obj = {};").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Object(props), .. } => {
            assert_eq!(props.len(), 0);
        }
        _ => panic!("Expected empty object"),
    }

    // Empty array
    let ast = parse("vl arr = [];").unwrap();
    match &ast.statements[0] {
        Statement::VarDecl { value: Expression::Array(elements), .. } => {
            assert_eq!(elements.len(), 0);
        }
        _ => panic!("Expected empty array"),
    }
}
