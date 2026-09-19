#[test]
fn syntax_codes_are_stable_and_distinct() {
    use hexput_shared::diagnostics::Code;
    assert_eq!(Code::EXPECTED_SYNTAX.as_str(), "syntax.expected_syntax");
    assert_eq!(
        Code::INVALID_ASSIGNMENT_TARGET.as_str(),
        "syntax.invalid_assignment_target"
    );
    assert_eq!(
        Code::DUPLICATE_DECLARATION.as_str(),
        "syntax.duplicate_declaration"
    );
}
