use hexput_ast::{Category, Code, Diagnostic};
use hexput_interpreter::{Value, evaluate};
use hexput_parser::parse;

fn eval(source: &str) -> Result<Value, Diagnostic> {
    let program = parse(source).unwrap_or_else(|e| panic!("`{source}` should parse: {e}"));
    evaluate(&program)
}

fn ok(source: &str) -> Value {
    eval(source).unwrap_or_else(|e| panic!("`{source}` should evaluate: {e}"))
}

fn err(source: &str) -> Diagnostic {
    match eval(source) {
        Ok(v) => panic!("`{source}` should fail, got {v:?}"),
        Err(e) => e,
    }
}

fn num(source: &str) -> f64 {
    ok(source)
        .as_number()
        .unwrap_or_else(|| panic!("`{source}` should yield a number"))
}

fn text(source: &str) -> String {
    ok(source)
        .as_str()
        .unwrap_or_else(|| panic!("`{source}` should yield a string"))
        .to_owned()
}

fn boolean(source: &str) -> bool {
    ok(source)
        .as_bool()
        .unwrap_or_else(|| panic!("`{source}` should yield a bool"))
}

/// Assert category, code, and that the span covers exactly the source text `spanned`.
fn assert_error(source: &str, category: Category, code: Code, spanned: &str) -> Diagnostic {
    let e = err(source);
    assert_eq!(e.category, category, "{source}: {e}");
    assert_eq!(e.code, code, "{source}: {e}");
    assert_eq!(&source[e.span.range()], spanned, "{source}: {e}");
    e
}

#[test]
fn values_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Value>();
}

#[test]
fn mixed_addition() {
    assert_eq!(text(r#"return "Total: " + 5;"#), "Total: 5");
    assert_eq!(text(r#"return "10" + 5;"#), "105");
    assert_eq!(num("return true + 1;"), 2.0);
    assert_eq!(text(r#"return "Order " + null;"#), "Order null");
    assert_eq!(text(r#"return "a" + true + false;"#), "atruefalse");
    assert_eq!(num("return null + 1;"), 1.0);
    assert_eq!(num("return 0.1 + 0.2;"), 0.1 + 0.2);
}

#[test]
fn arithmetic_coercion() {
    assert_eq!(num(r#"return "10" - 1;"#), 9.0);
    assert_eq!(num("return true * 3;"), 3.0);
    assert_eq!(num(r#"return " 2 " * 2;"#), 4.0);
    assert_eq!(num(r#"return "-3" - 1;"#), -4.0);
    assert_eq!(num(r#"return "+3" - 1;"#), 2.0);
    assert_eq!(num(r#"return "\n1.5e2\t" / 3;"#), 50.0);
    assert_eq!(num("return 7 % 3;"), 1.0);
    assert_eq!(num("return -7 % 3;"), -1.0);
    assert_eq!(num("return -true;"), -1.0);
    assert_eq!(num(r#"return -"4";"#), -4.0);
    assert_eq!(num("return 2 + 3 * 4 - 6 / 2;"), 11.0);
}

#[test]
fn bad_conversions_are_type_errors_naming_both_types() {
    let e = assert_error(
        r#"return "abc" * 2;"#,
        Category::Type,
        Code::OPERAND_MISMATCH,
        r#""abc""#,
    );
    assert!(e.message.contains("string") && e.message.contains("number"));
    let e = assert_error(
        "return [] - 1;",
        Category::Type,
        Code::OPERAND_MISMATCH,
        "[]",
    );
    assert!(e.message.contains("array") && e.message.contains("number"));
    let e = assert_error(
        r#"return "x" + [1];"#,
        Category::Type,
        Code::OPERAND_MISMATCH,
        "[1]",
    );
    assert!(e.message.contains("string") && e.message.contains("array"));
    let e = assert_error("return -{};", Category::Type, Code::OPERAND_MISMATCH, "{}");
    assert!(e.message.contains("object"));
    assert_error(
        r#"return {} + "x";"#,
        Category::Type,
        Code::OPERAND_MISMATCH,
        "{}",
    );
    assert_error(
        "return 1 + [];",
        Category::Type,
        Code::OPERAND_MISMATCH,
        "[]",
    );
    for bad in [
        "", " ", "NaN", "Infinity", "0x10", ".5", "5.", "1e", "--1", "- 1", "1e400",
    ] {
        let source = format!("return {bad:?} * 1;");
        let e = err(&source);
        assert_eq!(e.code, Code::OPERAND_MISMATCH, "{source}");
        assert_eq!(e.category, Category::Type, "{source}");
    }
}

#[test]
fn non_finite_results_are_arithmetic_errors() {
    assert_error(
        "return 1 / 0;",
        Category::Arithmetic,
        Code::DIVISION_BY_ZERO,
        "0",
    );
    assert_error(
        "return 0 % 0;",
        Category::Arithmetic,
        Code::DIVISION_BY_ZERO,
        "0",
    );
    let e = err(r#"return 1 / "0";"#);
    assert_eq!(e.code, Code::DIVISION_BY_ZERO);
    let e = err("return 1 / -0;");
    assert_eq!(e.code, Code::DIVISION_BY_ZERO);
    let source = "return 1e308 * 10;";
    let e = err(source);
    assert_eq!(e.category, Category::Arithmetic);
    assert_eq!(e.code, Code::NON_FINITE);
    assert_eq!(&source[e.span.range()], "*");
    assert_eq!(err("return 1e308 + 1e308;").code, Code::NON_FINITE);
    assert_eq!(err("return -1e308 - 1e308;").code, Code::NON_FINITE);
    assert_eq!(err("return 1e308 / 1e-308;").code, Code::NON_FINITE);
}

#[test]
fn equality() {
    assert!(!boolean("return 0 == false;"));
    assert!(!boolean(r#"return "" == false;"#));
    assert!(!boolean("return [] == false;"));
    assert!(boolean(r#"return "5" == 5;"#));
    assert!(boolean(r#"return 5 == " 5 ";"#));
    assert!(!boolean(r#"return "a" == 1;"#));
    assert!(boolean(r#"return "a" != 1;"#));
    assert!(boolean("return null == null;"));
    assert!(!boolean("return null == 0;"));
    assert!(!boolean(r#"return null == "";"#));
    assert!(boolean("let a = []; let b = a; return a == b;"));
    assert!(boolean("let a = [1]; return a == a;"));
    assert!(!boolean("return [] == [];"));
    assert!(!boolean("return {} == {};"));
    assert!(boolean("return [] != [];"));
    assert!(boolean(r#"return "x" == "x";"#));
    assert!(boolean("return true == true;"));
    assert!(boolean("return 0 == -0;"));
    assert!(!boolean(r#"return "1" == true;"#));
}

#[test]
fn ordering() {
    assert!(boolean(r#"return "b" > "a";"#));
    assert!(!boolean(r#"return "10" < 9;"#));
    assert!(boolean(r#"return "10" < "9";"#));
    assert!(boolean("return null < 1;"));
    assert!(boolean("return true >= 1;"));
    assert!(boolean("return 2 <= 2;"));
    assert!(boolean(r#"return "a" < "é";"#));
    let e = assert_error(
        r#"return "x" < 1;"#,
        Category::Type,
        Code::OPERAND_MISMATCH,
        r#""x""#,
    );
    assert!(e.message.contains("string") && e.message.contains("number"));
    assert_error(
        "return 1 < [];",
        Category::Type,
        Code::OPERAND_MISMATCH,
        "[]",
    );
}

#[test]
fn logic_short_circuits_and_returns_operands() {
    assert_eq!(text(r#"return null || "u";"#), "u");
    assert_eq!(text(r#"return "a" || nope;"#), "a");
    assert_eq!(num("return 0 && x_undeclared;"), 0.0);
    assert!(ok("return [] && nope;").as_array().is_some());
    assert_eq!(num("return 1 && 2;"), 2.0);
    assert_eq!(num("return 0 || 0 || 3;"), 3.0);
    assert!(ok("return {} || null;").is_null());
    assert!(boolean(r#"return !"";"#));
    assert!(!boolean(r#"return !"a";"#));
    assert!(boolean("return ![];"));
    assert!(!boolean("return ![0];"));
    assert!(boolean("return !{};"));
    assert!(boolean("return !null;"));
    assert!(boolean("return !0;"));
    assert!(!boolean("return !-1;"));
    // The decided side never runs; the undecided side does.
    assert_error(
        "return 1 && nope;",
        Category::Reference,
        Code::UNDECLARED_IDENTIFIER,
        "nope",
    );
    assert_error(
        "return 0 || nope;",
        Category::Reference,
        Code::UNDECLARED_IDENTIFIER,
        "nope",
    );
}

#[test]
fn operands_evaluate_left_to_right() {
    assert_error(
        "return first + second;",
        Category::Reference,
        Code::UNDECLARED_IDENTIFIER,
        "first",
    );
    assert_error(
        "return [a1, a2];",
        Category::Reference,
        Code::UNDECLARED_IDENTIFIER,
        "a1",
    );
    assert_error(
        "return {x: b1, y: b2};",
        Category::Reference,
        Code::UNDECLARED_IDENTIFIER,
        "b1",
    );
    // Assignment: receiver, then key, then value.
    assert_error(
        "u[k] = v;",
        Category::Reference,
        Code::UNDECLARED_IDENTIFIER,
        "u",
    );
    assert_error(
        "let o = {}; o[k] = v;",
        Category::Reference,
        Code::UNDECLARED_IDENTIFIER,
        "k",
    );
    assert_error(
        "let o = {}; o.p = v;",
        Category::Reference,
        Code::UNDECLARED_IDENTIFIER,
        "v",
    );
}

#[test]
fn shadowing_reads_inner_and_restores_outer() {
    let source = "let x = 1; let seen = 0; { let x = 2; seen = x; }; return [x, seen];";
    let result = ok(source);
    let items = result.as_array().expect("array").to_vec();
    assert_eq!(items[0].as_number(), Some(1.0));
    assert_eq!(items[1].as_number(), Some(2.0));
    // Assignment without `let` reaches the outer binding.
    assert_eq!(num("let x = 1; { x = 5; }; return x;"), 5.0);
    // An inner initializer reads the outer binding it shadows.
    assert_eq!(num("let x = 1; { let x = x + 1; return x; }"), 2.0);
    // Inner declarations do not leak out of their block.
    assert_error(
        "{ let inner = 1; }; return inner;",
        Category::Reference,
        Code::UNDECLARED_IDENTIFIER,
        "inner",
    );
}

#[test]
fn undeclared_names_are_reference_errors() {
    let e = assert_error(
        "return nope;",
        Category::Reference,
        Code::UNDECLARED_IDENTIFIER,
        "nope",
    );
    assert!(e.message.contains("nope"));
    let e = assert_error(
        "nope = 1;",
        Category::Reference,
        Code::UNDECLARED_ASSIGNMENT,
        "nope",
    );
    assert!(e.message.contains("nope"));
}

#[test]
fn null_access_is_a_reference_error_naming_the_null_link() {
    let e = assert_error(
        "let o = {c: null}; return o.c.name;",
        Category::Reference,
        Code::NULL_ACCESS,
        ".name",
    );
    assert!(e.message.contains("`c`"), "{}", e.message);
    let e = assert_error(
        "let a = null; return a.b;",
        Category::Reference,
        Code::NULL_ACCESS,
        ".b",
    );
    assert!(e.message.contains("`a`"));
    let e = assert_error(
        "let a = null; return a[nope];",
        Category::Reference,
        Code::NULL_ACCESS,
        "[nope]",
    );
    assert!(e.message.contains("`a`"));
    let e = assert_error(
        "let o = {c: null}; o.c.name = 1;",
        Category::Reference,
        Code::NULL_ACCESS,
        ".name",
    );
    assert!(e.message.contains("`c`"));
    assert_error(
        "let o = {c: null}; o.c[0] = 1;",
        Category::Reference,
        Code::NULL_ACCESS,
        "[0]",
    );
    // A null mid-way through an assignment target's receiver: `?.` is not allowed there, so
    // the message must not suggest it.
    let e = assert_error(
        "let o = {c: null}; o.c.d.e = 1;",
        Category::Reference,
        Code::NULL_ACCESS,
        ".d",
    );
    assert!(e.message.contains("`c`"), "{}", e.message);
    assert!(!e.message.contains("?."), "{}", e.message);
    let e = assert_error(
        "let o = {c: null}; o.c[0].e = 1;",
        Category::Reference,
        Code::NULL_ACCESS,
        "[0]",
    );
    assert!(!e.message.contains("?."), "{}", e.message);
    // Plain reads still suggest it.
    let e = err("let o = {c: null}; return o.c.d;");
    assert!(e.message.contains("?."), "{}", e.message);
    assert_error(
        "return null.x;",
        Category::Reference,
        Code::NULL_ACCESS,
        ".x",
    );
}

#[test]
fn absent_data_reads_null_and_writes_create() {
    assert!(ok("let o = {}; return o.missing;").is_null());
    assert!(ok("return [1][5];").is_null());
    assert!(ok("return [1][-1];").is_null());
    assert!(ok("return [1][0.5];").is_null());
    assert!(ok(r#"return {a: 1}["b"];"#).is_null());
    assert_eq!(num(r#"return {a: 1}["a"];"#), 1.0);
    let result = ok("let o = {a: 1, b: 2}; o.k = 1; o.a = 3; o[\"z\"] = 4; return o;");
    let entries = result.as_object().expect("object").entries();
    let keys: Vec<&str> = entries.iter().map(|(k, _)| k.as_ref()).collect();
    assert_eq!(keys, ["a", "b", "k", "z"]);
    assert_eq!(entries[0].1.as_number(), Some(3.0));
    assert_eq!(entries[2].1.as_number(), Some(1.0));
    // Every literal value pairs with its own key.
    let items = ok("let o = {a: 1, b: 2, c: 3}; return [o.a, o.b, o.c];")
        .as_array()
        .expect("array")
        .to_vec();
    let numbers: Vec<f64> = items.iter().filter_map(Value::as_number).collect();
    assert_eq!(numbers, [1.0, 2.0, 3.0]);
    // Collections are shared by reference.
    assert_eq!(num("let a = {n: 1}; let b = a; b.n = 2; return a.n;"), 2.0);
    assert_eq!(num("let o = {p: {q: 1}}; o.p.q = 7; return o.p.q;"), 7.0);
}

#[test]
fn array_writes() {
    let items = ok("let a = [1, 2]; a[0] = 9; a[2] = 3; return a;")
        .as_array()
        .expect("array")
        .to_vec();
    let numbers: Vec<f64> = items.iter().filter_map(Value::as_number).collect();
    assert_eq!(numbers, [9.0, 2.0, 3.0]);
    for (source, spanned) in [
        ("let a = [1]; a[3] = 0;", "3"),
        ("let a = [1]; a[-1] = 0;", "-1"),
        ("let a = [1]; a[0.5] = 0;", "0.5"),
        ("let a = []; a[1] = 0;", "1"),
    ] {
        assert_error(
            source,
            Category::Reference,
            Code::INDEX_OUT_OF_RANGE,
            spanned,
        );
    }
    assert_error(
        r#"let a = []; a["0"] = 1;"#,
        Category::Type,
        Code::INVALID_INDEX,
        r#""0""#,
    );
    assert_error(
        "let o = {}; o[0] = 1;",
        Category::Type,
        Code::INVALID_INDEX,
        "0",
    );
    assert_error(
        "let s = \"ab\"; s[0] = 1;",
        Category::Type,
        Code::INVALID_INDEX,
        "[0]",
    );
    assert_error(
        "let n = 1; n.x = 1;",
        Category::Type,
        Code::INVALID_PROPERTY_ACCESS,
        ".x",
    );
    assert_error(
        "let a = []; a.x = 1;",
        Category::Type,
        Code::INVALID_PROPERTY_ACCESS,
        ".x",
    );
}

#[test]
fn optional_access() {
    assert!(ok("let o = {c: null}; return o.c?.name;").is_null());
    assert!(ok("let a = null; return a?.b.c.d;").is_null());
    assert!(ok("let a = null; return a?.[f];").is_null());
    assert!(ok("let a = null; return a?.[f].g[h];").is_null());
    assert_eq!(num("let a = {b: {c: 4}}; return a?.b.c;"), 4.0);
    assert_eq!(num("let a = [5]; return a?.[0];"), 5.0);
    // Grouping ends the chain the `?.` short-circuits.
    assert_error(
        "let a = null; return (a?.b).c;",
        Category::Reference,
        Code::NULL_ACCESS,
        ".c",
    );
    // `?.` suppresses only null, never type errors.
    assert_error(
        "return 1?.x;",
        Category::Type,
        Code::INVALID_PROPERTY_ACCESS,
        "?.x",
    );
    assert_error(
        "return [1]?.[\"0\"];",
        Category::Type,
        Code::INVALID_INDEX,
        "\"0\"",
    );
    // A non-null link after `?.` still raises on a later null.
    assert_error(
        "let a = {b: null}; return a?.b.c;",
        Category::Reference,
        Code::NULL_ACCESS,
        ".c",
    );
}

#[test]
fn index_and_property_types() {
    assert_error(
        r#"return [1]["0"];"#,
        Category::Type,
        Code::INVALID_INDEX,
        r#""0""#,
    );
    assert_error(
        r#"return "ab"[0];"#,
        Category::Type,
        Code::INVALID_INDEX,
        "[0]",
    );
    assert_error(
        "return {a: 1}[0];",
        Category::Type,
        Code::INVALID_INDEX,
        "0",
    );
    assert_error(
        "return true[0];",
        Category::Type,
        Code::INVALID_INDEX,
        "[0]",
    );
    for (source, receiver) in [
        ("let n = 5; return n.x;", "number"),
        ("return true.x;", "bool"),
        (r#"return "s".length;"#, "string"),
        ("return [1].length;", "array"),
    ] {
        let e = err(source);
        assert_eq!(e.category, Category::Type, "{source}");
        assert_eq!(e.code, Code::INVALID_PROPERTY_ACCESS, "{source}");
        assert!(e.message.contains(receiver), "{source}: {}", e.message);
    }
    assert_eq!(num("return [[1, 2], [3]][0][1];"), 2.0);
    assert_eq!(num("return {a: [1, {b: 6}]}.a[1].b;"), 6.0);
}

#[test]
fn script_result() {
    assert!(ok("let x = 1;").is_null());
    assert!(ok("").is_null());
    assert!(ok("return;").is_null());
    assert!(ok("let x = 1; return").is_null());
    assert_eq!(num("{ { return 3; nope; }; nope; }; return 4;"), 3.0);
    assert_eq!(num("let x = 1; { x = 2; return x; }"), 2.0);
    assert_eq!(num("return 1; return 2;"), 1.0);
}

#[test]
fn number_to_string_is_javascript_style() {
    for (literal, expected) in [
        ("5", "5"),
        ("2.5", "2.5"),
        ("-0", "0"),
        ("-12", "-12"),
        ("(0.1 + 0.2)", "0.30000000000000004"),
        ("1e21", "1e+21"),
        ("1.5e21", "1.5e+21"),
        ("123456789012345680000", "123456789012345680000"),
        ("1e-7", "1e-7"),
        ("-1.5e-7", "-1.5e-7"),
        ("0.000001", "0.000001"),
        ("0.00001234", "0.00001234"),
        ("1e300", "1e+300"),
        ("100", "100"),
        ("(1 / 3)", "0.3333333333333333"),
    ] {
        assert_eq!(
            text(&format!(r#"return "" + {literal};"#)),
            expected,
            "{literal}"
        );
    }
}

#[test]
fn constructs_owned_by_story_1_7_fail_without_panicking() {
    for source in [
        "if (1) {}",
        "while (0) {}",
        "for (x in []) {}",
        "fn f() {}",
        "let f = fn() {};",
        "let f = 1; f();",
        "let o = {}; o.m();",
    ] {
        let e = err(source);
        assert_eq!(e.code.as_str(), "temporary.not_yet_implemented", "{source}");
    }
    // An optional chain that short-circuits never reaches its call.
    assert!(ok("let a = null; return a?.b();").is_null());
}

#[test]
fn deep_input_is_stack_safe() {
    let n = 12_000;
    let groups = format!("return {}1{};", "(".repeat(n), ")".repeat(n));
    assert_eq!(num(&groups), 1.0);
    let sum = format!("return 1{};", "+1".repeat(n));
    assert_eq!(num(&sum), (n + 1) as f64);
    let right = format!("return {}1{};", "1+(".repeat(n), ")".repeat(n));
    assert_eq!(num(&right), (n + 1) as f64);
    let nots = format!("return {}0;", "!".repeat(n));
    assert!(!boolean(&nots)); // an even count of `!` on falsy 0
    let blocks = format!("let x = 7; {}return x;{}", "{".repeat(n), "}".repeat(n));
    assert_eq!(num(&blocks), 7.0);
    let shadowed = format!("{}return 1;{}", "{let x = 1;".repeat(n), "}".repeat(n));
    assert_eq!(num(&shadowed), 1.0);
    let failing = format!("{}nope;{}", "{let x = 1;".repeat(n), "}".repeat(n));
    assert_eq!(err(&failing).code, Code::UNDECLARED_IDENTIFIER);
    let nested_object = format!(
        "let o = {}null{}; return o{};",
        "{b:".repeat(n),
        "}".repeat(n),
        ".b".repeat(n)
    );
    assert!(ok(&nested_object).is_null());
    let too_far = format!(
        "let o = {}null{}; return o{};",
        "{b:".repeat(n),
        "}".repeat(n),
        ".b".repeat(n + 1)
    );
    assert_eq!(err(&too_far).code, Code::NULL_ACCESS);
    let nested_array = format!("return {}{};", "[".repeat(n), "]".repeat(n));
    let value = ok(&nested_array);
    assert_eq!(
        value.as_array().map(hexput_interpreter::Array::len),
        Some(1)
    );
    drop(value);
    let indices = format!("let a = [0]; return {}0{};", "a[".repeat(n), "]".repeat(n));
    assert_eq!(num(&indices), 0.0);
    let optional = format!("let a = null; return a?.b{};", ".c".repeat(n));
    assert!(ok(&optional).is_null());
}

#[test]
fn short_token_combinations_do_not_panic() {
    let atoms = [
        "let", "x", "1", "\"s\"", "[", "]", "{", "}", ".", "?.", "+", "/", "=", ";", "null",
        "return",
    ];
    for a in atoms {
        for b in atoms {
            for c in atoms {
                for d in atoms {
                    if let Ok(program) = parse(&format!("{a} {b} {c} {d}")) {
                        let _ = evaluate(&program);
                    }
                }
            }
        }
    }
}
