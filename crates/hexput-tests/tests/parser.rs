use hexput_ast::*;
use hexput_parser::parse;

fn root(p: &Program) -> ExprId {
    let StatementKind::Expression(id) = p.statements[0].kind else {
        panic!("expected expression")
    };
    id
}
fn binary(p: &Program, id: ExprId) -> (ExprId, BinaryOperator, ExprId) {
    let ExpressionKind::Binary {
        left,
        operator,
        right,
    } = p.expression(id).kind
    else {
        panic!("expected binary")
    };
    (left, operator.kind, right)
}

#[test]
fn statement_order_values_and_precedence() {
    let source = "let x = 1 + 2 * 3; x = (x - 1) / 2";
    let p = parse(source).unwrap();
    assert_eq!(p.span, Span::new(0, source.len(), 1, 1));
    assert_eq!(p.statements.len(), 2);
    let StatementKind::Let {
        keyword,
        name,
        equals,
        initializer,
    } = &p.statements[0].kind
    else {
        panic!()
    };
    assert_eq!(*keyword, Span::new(0, 3, 1, 1));
    assert_eq!(
        name,
        &Identifier {
            name: "x".into(),
            span: Span::new(4, 1, 1, 5)
        }
    );
    assert_eq!(*equals, Span::new(6, 1, 1, 7));
    assert_eq!(p.statements[0].span, Span::new(0, 18, 1, 1));
    assert_eq!(p.statements[0].terminator, Some(Span::new(17, 1, 1, 18)));
    let (one, op, product) = binary(&p, *initializer);
    assert_eq!(op, BinaryOperator::Add);
    assert_eq!(
        p.expression(one).kind,
        ExpressionKind::Literal(Literal::Number(1.0))
    );
    assert_eq!(binary(&p, product).1, BinaryOperator::Multiply);
    let StatementKind::Assignment {
        target,
        equals,
        value,
    } = p.statements[1].kind
    else {
        panic!()
    };
    assert_eq!(&source[p.expression(target).span.range()], "x");
    assert_eq!(&source[equals.range()], "=");
    let (group, op, _) = binary(&p, value);
    assert_eq!(op, BinaryOperator::Divide);
    let ExpressionKind::Group { expression, .. } = p.expression(group).kind else {
        panic!()
    };
    assert_eq!(binary(&p, expression).1, BinaryOperator::Subtract);
    assert_eq!(p.statements[1].terminator, None);
}

#[test]
fn every_binary_operator_pair_obeys_precedence_and_left_associativity() {
    use BinaryOperator::*;
    let operators = [
        ("||", Or, 1),
        ("&&", And, 2),
        ("==", Equal, 3),
        ("!=", NotEqual, 3),
        ("<", Less, 4),
        ("<=", LessEqual, 4),
        (">", Greater, 4),
        (">=", GreaterEqual, 4),
        ("+", Add, 5),
        ("-", Subtract, 5),
        ("*", Multiply, 6),
        ("/", Divide, 6),
        ("%", Remainder, 6),
    ];
    for (a, first, pa) in operators {
        for (b, second, pb) in operators {
            let source = format!("a {a} b {b} c");
            let p = parse(&source).unwrap();
            let (left, op, right) = binary(&p, root(&p));
            if pa < pb {
                assert_eq!(op, first, "{source}");
                assert_eq!(binary(&p, right).1, second);
            } else {
                assert_eq!(op, second, "{source}");
                assert_eq!(binary(&p, left).1, first);
            }
            for expression in &p.expressions {
                if let ExpressionKind::Binary { operator, .. } = expression.kind {
                    assert!([a, b].contains(&&source[operator.span.range()]));
                }
            }
        }
    }
}

#[test]
fn unary_access_and_groups_preserve_chain_boundaries() {
    let p = parse("!-a.b[0]").unwrap();
    let ExpressionKind::Unary { operator, operand } = p.expression(root(&p)).kind else {
        panic!()
    };
    assert_eq!(
        operator,
        Spanned {
            kind: UnaryOperator::Not,
            span: Span::new(0, 1, 1, 1)
        }
    );
    let ExpressionKind::Unary { operator, operand } = p.expression(operand).kind else {
        panic!()
    };
    assert_eq!(operator.kind, UnaryOperator::Negate);
    let ExpressionKind::Access { links, .. } = &p.expression(operand).kind else {
        panic!()
    };
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].span, Span::new(3, 2, 1, 4));
    assert_eq!(links[1].span, Span::new(5, 3, 1, 6));

    let source = "a?.b.c?.[i + 1]";
    let p = parse(source).unwrap();
    let ExpressionKind::Access { links, .. } = &p.expression(root(&p)).kind else {
        panic!()
    };
    assert_eq!(
        links.iter().map(|l| l.optional).collect::<Vec<_>>(),
        [true, false, true]
    );
    assert_eq!(
        links
            .iter()
            .map(|l| &source[l.span.range()])
            .collect::<Vec<_>>(),
        ["?.b", ".c", "?.[i + 1]"]
    );
    assert_eq!(links[2].operator, Span::new(6, 2, 1, 7));
    let AccessKind::Index {
        open,
        expression,
        close,
    } = links[2].kind
    else {
        panic!()
    };
    assert_eq!(open, Span::new(8, 1, 1, 9));
    assert_eq!(close, Span::new(14, 1, 1, 15));
    assert_eq!(binary(&p, expression).1, BinaryOperator::Add);

    let p = parse("(a?.b).c").unwrap();
    let ExpressionKind::Access { base, links } = &p.expression(root(&p)).kind else {
        panic!()
    };
    assert_eq!(links.len(), 1);
    let ExpressionKind::Group {
        open,
        expression,
        close,
    } = p.expression(*base).kind
    else {
        panic!()
    };
    assert_eq!(open, Span::new(0, 1, 1, 1));
    assert_eq!(close, Span::new(5, 1, 1, 6));
    assert!(
        matches!(&p.expression(expression).kind, ExpressionKind::Access { links, .. } if links[0].optional)
    );
}

#[test]
fn scalars_and_empty_programs() {
    for source in ["", " \r\n /* ğ */ // x", "\u{feff}"] {
        let p = parse(source).unwrap();
        assert!(p.statements.is_empty());
        assert!(p.expressions.is_empty());
    }
    let p = parse("null; true; false; 1e3; 'a\\nğ'; missing").unwrap();
    let expected = [
        Literal::Null,
        Literal::Bool(true),
        Literal::Bool(false),
        Literal::Number(1000.0),
        Literal::String("a\nğ".into()),
    ];
    for (statement, value) in p.statements.iter().zip(expected) {
        let StatementKind::Expression(id) = statement.kind else {
            panic!()
        };
        assert_eq!(p.expression(id).kind, ExpressionKind::Literal(value));
    }
    assert_eq!(p.statements.len(), 6);
}

#[test]
fn assignments_and_declaration_rules() {
    let source = "obj.key = 1; arr[i] = a?.b";
    let p = parse(source).unwrap();
    assert_eq!(p.statements.len(), 2);
    for (statement, expected_target, expected_value) in [
        (&p.statements[0], "obj.key", "1"),
        (&p.statements[1], "arr[i]", "a?.b"),
    ] {
        let StatementKind::Assignment {
            target,
            equals,
            value,
        } = statement.kind
        else {
            panic!("expected an assignment");
        };
        assert_eq!(&source[p.expression(target).span.range()], expected_target);
        assert_eq!(&source[p.expression(value).span.range()], expected_value);
        assert_eq!(&source[equals.range()], "=");
        let ExpressionKind::Access { links, .. } = &p.expression(target).kind else {
            panic!("expected an access target");
        };
        assert_eq!(links.len(), 1);
        assert!(!links[0].optional);
        if expected_target == "obj.key" {
            assert!(matches!(&links[0].kind, AccessKind::Property(name) if name.name == "key"));
        } else {
            let AccessKind::Index { expression, .. } = links[0].kind else {
                panic!("expected an index target");
            };
            assert!(
                matches!(&p.expression(expression).kind, ExpressionKind::Identifier(name) if name.name == "i")
            );
            assert!(
                matches!(&p.expression(value).kind, ExpressionKind::Access { links, .. } if links.len() == 1 && links[0].optional)
            );
        }
    }
    for source in [
        "obj.key = 1; arr[i] = a?.b",
        "a[b?.c] = 1",
        "a[b?.[c]].d = 1",
        "(a?.b).c = 1",
        "missing = 1",
        "let x = x; x",
        "let x = 1;",
    ] {
        assert!(parse(source).is_ok(), "{source}");
    }
    for source in [
        "1 = 2",
        "a + b = 3",
        "a?.b.c = 1",
        "a?.[b] = 1",
        "(a) = 1",
        "!a = 1",
    ] {
        assert_eq!(
            parse(source).unwrap_err().code,
            Code::INVALID_ASSIGNMENT_TARGET,
            "{source}"
        );
    }
    let error = parse("let x = 1; let x = 2").unwrap_err();
    assert_eq!(error.code, Code::DUPLICATE_DECLARATION);
    assert_eq!(error.span, Span::new(15, 1, 1, 16));
    assert!(parse("let x = 1; let X = 2").is_ok());
}

#[test]
fn malformed_and_unsupported_input_is_never_partially_accepted() {
    for source in [
        "let x;",
        "let if = 1",
        "let x = 1 let y = 2",
        "(1 + 2",
        "a[]",
        "a.",
        "a?.",
        "1 +",
        "a = b = 1",
        "a b",
        "a; ;",
        ";",
        "a[1)",
        "(1]",
        "a[1;",
        "()",
        "a?.()",
        "f()",
        "[1]",
        "return 1",
        "fn f() {}",
        "plugin {}",
        "@Event(x)",
        "a.true",
        "a?.null",
        "a.[1]",
        "a)",
        "a]",
        "a[1 2]",
        "a + (b * )",
    ] {
        let error = parse(source).unwrap_err();
        assert_eq!(error.category, Category::Syntax, "{source}");
        assert_eq!(error.code, Code::EXPECTED_SYNTAX, "{source}");
        assert!(
            error.message.contains("expected") && error.message.contains("found"),
            "{source}"
        );
        assert!(error.span.end() <= source.len());
    }
    let e = parse("let if = 1").unwrap_err();
    assert_eq!(e.span, Span::new(4, 2, 1, 5));
    assert!(e.message.contains("`if`"));

    for (source, span, expected) in [
        ("let x;", Span::new(5, 1, 1, 6), "`=` and an initializer"),
        (
            "let x = 1 let y = 2",
            Span::new(10, 3, 1, 11),
            "`;` between statements",
        ),
        ("(1 + 2", Span::new(6, 0, 1, 7), "`)` to close the group"),
        ("a[]", Span::new(2, 1, 1, 3), "an expression"),
        ("a.", Span::new(2, 0, 1, 3), "an identifier"),
        ("a?.", Span::new(3, 0, 1, 4), "an identifier"),
        ("1 +", Span::new(3, 0, 1, 4), "an expression"),
    ] {
        let error = parse(source).unwrap_err();
        assert_eq!(error.span, span, "{source}");
        assert!(
            error.message.contains(expected),
            "{source}: {}",
            error.message
        );
    }
}

#[test]
fn exact_positions_follow_lexer_through_unicode_bom_and_trivia() {
    let source = "\u{feff}// ğ\r\nlet x = 'ö\n中';\rx + /* ğ */\r\n";
    let error = parse(source).unwrap_err();
    assert_eq!(error.span, Span::new(source.len(), 0, 5, 1));
    assert!(error.message.contains("end of input"));
    let source = "\u{feff}'ö\n中';\rx.abc // tail";
    let p = parse(source).unwrap();
    assert_eq!(p.expression(root(&p)).span, Span::new(3, 8, 1, 1));
    let StatementKind::Expression(id) = p.statements[1].kind else {
        panic!()
    };
    assert_eq!(p.expression(id).span, Span::new(13, 5, 3, 1));
    let ExpressionKind::Access { links, .. } = &p.expression(id).kind else {
        panic!()
    };
    let AccessKind::Property(name) = &links[0].kind else {
        panic!()
    };
    assert_eq!(name.span, Span::new(15, 3, 3, 3));
    for (source, line, column) in [
        ("1 + // 中", 1, 9),
        ("1 +\r", 2, 1),
        ("\u{feff}1 +", 1, 4),
        ("1 +\r\n ", 2, 2),
    ] {
        assert_eq!(
            parse(source).unwrap_err().span,
            Span::new(source.len(), 0, line, column)
        );
    }
}

#[test]
fn lexical_errors_are_propagated_unchanged() {
    for source in ["'x", "1e", "a & b", "let = 1; /*", "'\\z'", "café"] {
        assert_eq!(
            parse(source).unwrap_err(),
            hexput_lexer::tokenize(source).unwrap_err()
        );
    }
}

#[test]
fn deep_and_long_inputs_parse_clone_compare_and_drop_without_recursion() {
    let n = 20_000;
    for source in [
        format!("{}1{}", "(".repeat(n), ")".repeat(n)),
        format!("{}1", "!".repeat(n)),
        format!("a{}", "+a".repeat(n)),
        format!("a{}", ".b".repeat(n)),
        format!("{}0{}", "a[".repeat(n), "]".repeat(n)),
    ] {
        let p = parse(&source).unwrap();
        assert_eq!(p.expression(root(&p)).span.len, source.len());
        assert_eq!(p, p.clone());
        drop(p);
    }
    for source in [
        format!("{}1", "(".repeat(n)),
        "!".repeat(n),
        format!("a{}+", "+a".repeat(n)),
        format!("{}0", "a[".repeat(n)),
    ] {
        assert!(parse(&source).is_err());
    }
}

#[test]
fn short_token_combinations_do_not_panic() {
    // Sample all four-token sequences from this limited alphabet; longer and nested
    // transitions need their own structural and rejection tests.
    let atoms = ["a", "1", "!", "+", "(", ")", "[", "]", ".", "?.", "=", ";"];
    for a in atoms {
        for b in atoms {
            for c in atoms {
                for d in atoms {
                    let _ = parse(&format!("{a} {b} {c} {d}"));
                }
            }
        }
    }
}

fn assert_name(p: &Program, id: ExprId, expected: &str) {
    assert!(id.0 < p.expressions.len());
    let ExpressionKind::Identifier(name) = &p.expression(id).kind else {
        panic!("expected name {expected}")
    };
    assert_eq!(name.name, expected);
}

fn unary(p: &Program, id: ExprId, expected: UnaryOperator) -> ExprId {
    let ExpressionKind::Unary { operator, operand } = p.expression(id).kind else {
        panic!("expected unary")
    };
    assert_eq!(operator.kind, expected);
    operand
}

#[test]
fn unary_binary_and_grouped_access_tree_shapes() {
    let p = parse("-a * b").unwrap();
    let (left, op, right) = binary(&p, root(&p));
    assert_eq!(op, BinaryOperator::Multiply);
    assert_name(&p, unary(&p, left, UnaryOperator::Negate), "a");
    assert_name(&p, right, "b");

    let p = parse("a * -b + c").unwrap();
    let (left, op, right) = binary(&p, root(&p));
    assert_eq!(op, BinaryOperator::Add);
    assert_name(&p, right, "c");
    let (left, op, right) = binary(&p, left);
    assert_eq!(op, BinaryOperator::Multiply);
    assert_name(&p, left, "a");
    assert_name(&p, unary(&p, right, UnaryOperator::Negate), "b");

    let p = parse("!a == b").unwrap();
    let (left, op, right) = binary(&p, root(&p));
    assert_eq!(op, BinaryOperator::Equal);
    assert_name(&p, unary(&p, left, UnaryOperator::Not), "a");
    assert_name(&p, right, "b");

    let p = parse("!(a == b)").unwrap();
    let group = unary(&p, root(&p), UnaryOperator::Not);
    let ExpressionKind::Group { expression, .. } = p.expression(group).kind else {
        panic!()
    };
    let (left, op, right) = binary(&p, expression);
    assert_eq!(op, BinaryOperator::Equal);
    assert_name(&p, left, "a");
    assert_name(&p, right, "b");

    let p = parse("(-a).b").unwrap();
    let ExpressionKind::Access { base, links } = &p.expression(root(&p)).kind else {
        panic!()
    };
    assert_eq!(links.len(), 1);
    assert!(!links[0].optional);
    assert!(matches!(&links[0].kind, AccessKind::Property(name) if name.name == "b"));
    let ExpressionKind::Group { expression, .. } = p.expression(*base).kind else {
        panic!()
    };
    assert_name(&p, unary(&p, expression, UnaryOperator::Negate), "a");
}

#[test]
fn nested_indices_and_forward_links_resolve_their_actual_receivers() {
    let p = parse("a[b[c] + d].e").unwrap();
    let ExpressionKind::Access { base, links } = &p.expression(root(&p)).kind else {
        panic!()
    };
    assert_name(&p, *base, "a");
    assert_eq!(links.len(), 2);
    assert!(links.iter().all(|link| !link.optional));
    assert!(matches!(&links[1].kind, AccessKind::Property(name) if name.name == "e"));
    let AccessKind::Index { expression, .. } = links[0].kind else {
        panic!()
    };
    let (left, op, right) = binary(&p, expression);
    assert_eq!(op, BinaryOperator::Add);
    assert_name(&p, right, "d");
    let ExpressionKind::Access { base, links } = &p.expression(left).kind else {
        panic!()
    };
    assert_name(&p, *base, "b");
    assert_eq!(links.len(), 1);
    assert!(!links[0].optional);
    let AccessKind::Index { expression, .. } = links[0].kind else {
        panic!()
    };
    assert_name(&p, expression, "c");

    let p = parse("a.b[c]").unwrap();
    let chain = root(&p);
    let ExpressionKind::Access { base, links } = &p.expression(chain).kind else {
        panic!()
    };
    assert_name(&p, *base, "a");
    assert_eq!(links.len(), 2);
    assert!(links.iter().all(|link| !link.optional));
    assert!(matches!(&links[0].kind, AccessKind::Property(name) if name.name == "b"));
    let AccessKind::Index { expression, .. } = links[1].kind else {
        panic!()
    };
    assert!(
        expression.0 > chain.0,
        "index payload is a forward reference"
    );
    assert_name(&p, expression, "c");
}

#[test]
fn errors_cover_complete_targets_and_exact_malformed_suffixes() {
    for (source, span) in [
        ("a?.b.c = 1", Span::new(0, 6, 1, 1)),
        ("a + b = 3", Span::new(0, 5, 1, 1)),
    ] {
        let error = parse(source).unwrap_err();
        assert_eq!(error.category, Category::Syntax);
        assert_eq!(error.code, Code::INVALID_ASSIGNMENT_TARGET);
        assert_eq!(error.span, span);
    }
    for (source, span, message) in [
        (
            "a + b[c)",
            Span::new(7, 1, 1, 8),
            "expected `]` to close the index, found `)`",
        ),
        (
            "a[b] +",
            Span::new(6, 0, 1, 7),
            "expected an expression (literal, identifier, `(`, `-`, or `!`), found end of input",
        ),
        (
            "(a[b]).",
            Span::new(7, 0, 1, 8),
            "expected an identifier, found end of input",
        ),
    ] {
        let error = parse(source).unwrap_err();
        assert_eq!(error.category, Category::Syntax);
        assert_eq!(error.code, Code::EXPECTED_SYNTAX);
        assert_eq!(error.span, span);
        assert_eq!(error.message, message);
    }
}

#[test]
fn oversized_unicode_token_preview_is_bounded_escaped_and_keeps_full_span() {
    let token = format!("'{}'", "中\n\t".repeat(500_000));
    let source = format!("let {token} = 1");
    let error = parse(&source).unwrap_err();
    assert_eq!(error.category, Category::Syntax);
    assert_eq!(error.code, Code::EXPECTED_SYNTAX);
    assert_eq!(error.span, Span::new(4, token.len(), 1, 5));
    // One opening quote and 21 three-scalar repetitions exhaust the 64-scalar preview.
    let preview = format!("'{}", "中\n\t".repeat(21));
    assert_eq!(
        error.message,
        format!(
            "expected an identifier, found `{}… (truncated)`",
            preview.escape_debug()
        )
    );
    assert!(!error.message.contains('\n'));
    assert!(!error.message.contains('\t'));
    assert!(error.message.len() < 256);
    assert_eq!(&source[error.span.range()], token);
}

#[test]
fn conditional_branches_and_loops_preserve_structure_and_spans() {
    let source = "if (x) {} else if (y) {1} else {2}; for (item in source?.items) {while (item) {if (y) {break} else {continue}}}";
    let p = parse(source).unwrap();
    let StatementKind::If {
        branches,
        else_branch,
    } = &p.statements[0].kind
    else {
        panic!()
    };
    assert_eq!(branches.len(), 2);
    assert_name(&p, branches[0].condition.expression, "x");
    assert_name(&p, branches[1].condition.expression, "y");
    assert_eq!(&source[branches[0].keyword.range()], "if");
    assert_eq!(branches[0].else_keyword, None);
    assert_eq!(&source[branches[1].else_keyword.unwrap().range()], "else");
    assert_eq!(&source[branches[1].condition.open.range()], "(");
    assert_eq!(&source[branches[1].condition.close.range()], ")");
    for (body, text) in [
        (branches[0].body, "{}"),
        (branches[1].body, "{1}"),
        (else_branch.as_ref().unwrap().body, "{2}"),
    ] {
        let b = p.block(body);
        assert_eq!(&source[b.span.range()], text);
        assert_eq!(&source[b.open.range()], "{");
        assert_eq!(&source[b.close.range()], "}");
    }
    assert_eq!(&source[p.statements[0].terminator.unwrap().range()], ";");
    let StatementKind::For {
        keyword,
        open,
        binding,
        in_keyword,
        iterable,
        close,
        body,
    } = &p.statements[1].kind
    else {
        panic!()
    };
    assert_eq!(&source[keyword.range()], "for");
    assert_eq!(&source[open.range()], "(");
    assert_eq!(&source[close.range()], ")");
    assert_eq!(&source[in_keyword.range()], "in");
    assert_eq!(&source[binding.span.range()], "item");
    assert_eq!(
        &source[p.expression(*iterable).span.range()],
        "source?.items"
    );
    let StatementKind::While {
        body, condition, ..
    } = &p.block(*body).statements[0].kind
    else {
        panic!()
    };
    assert_name(&p, condition.expression, "item");
    let StatementKind::If {
        branches,
        else_branch,
    } = &p.block(*body).statements[0].kind
    else {
        panic!()
    };
    assert_name(&p, branches[0].condition.expression, "y");
    assert!(matches!(
        p.block(branches[0].body).statements[0].kind,
        StatementKind::Break { .. }
    ));
    assert!(matches!(
        p.block(else_branch.as_ref().unwrap().body).statements[0].kind,
        StatementKind::Continue { .. }
    ));
}

#[test]
fn block_bindings_shadow_and_restore_without_leaking() {
    for source in [
        "let x = 0; if (x) {let x = 1} else {let x = 2}",
        "for (x in xs) {{let x = 1}; continue}; let x = 2",
        "{let y = 1}; {let y = 2}; let y = 3",
        "while (x) {for (x in xs) {break}; continue}",
    ] {
        parse(source).unwrap();
    }
    for source in [
        "while (x) {let y = 1; let y = 2}",
        "for (x in xs) {let x = 1}",
        "let x = 0; {let x = 1}; let x = 2",
    ] {
        let e = parse(source).unwrap_err();
        let offset = source.rfind("let ").unwrap() + 4;
        assert_eq!(e.code, Code::DUPLICATE_DECLARATION);
        assert_eq!(e.span, Span::new(offset, 1, 1, offset + 1));
    }
    for source in [
        "break",
        "if (x) {continue}",
        "while (x) {}; break",
        "{continue}",
    ] {
        let e = parse(source).unwrap_err();
        let name = if source.contains("continue") {
            "continue"
        } else {
            "break"
        };
        let offset = source.find(name).unwrap();
        assert_eq!(e.code, Code::LOOP_CONTROL_OUTSIDE_LOOP);
        assert_eq!(e.span, Span::new(offset, name.len(), 1, offset + 1));
    }
}

#[test]
fn malformed_flow_reports_construct_and_exact_token() {
    for (source, token, label) in [
        ("if () {}", ")", "if"),
        ("while x {}", "x", "while"),
        ("for (let x in xs) {}", "let", "for"),
        ("for (x xs) {}", "xs", "for"),
        ("for (x in ) {}", ")", "for"),
        ("if (x) 1", "1", "if"),
        ("if (x) {} else 1", "1", "else"),
        ("else {}", "else", "if"),
    ] {
        let e = parse(source).unwrap_err();
        let offset = source.find(token).unwrap();
        assert_eq!(e.category, Category::Syntax);
        assert_eq!(e.code, Code::EXPECTED_SYNTAX);
        assert_eq!(e.span, Span::new(offset, token.len(), 1, offset + 1));
        assert!(e.message.contains(label), "{}", e.message);
    }
    for (source, offset, len, context) in [
        ("if (x) {} x", 10, 1, "after if"),
        ("while (x) {} continue", 13, 8, "after while"),
        ("if (x) {}; else {}", 11, 4, "if branch before `else`"),
        ("if (x) {} else {} else {}", 18, 4, "after if"),
        ("if (x) {1 2}", 10, 1, "between statements"),
        ("for (x in xs) {break 1}", 21, 1, "after break"),
    ] {
        let e = parse(source).unwrap_err();
        assert_eq!(e.code, Code::EXPECTED_SYNTAX);
        assert_eq!(e.span, Span::new(offset, len, 1, offset + 1), "{source}");
        assert!(e.message.contains(context), "{}", e.message);
    }
    let source = "\u{feff}while (\"ğ\") {\r\n // tail\r\n";
    let e = parse(source).unwrap_err();
    assert_eq!(e.span, Span::new(source.len(), 0, 3, 1));
    assert!(e.message.contains("while"));
    assert_eq!(
        parse("if (x) { $ }").unwrap_err(),
        hexput_lexer::tokenize("if (x) { $ }").unwrap_err()
    );
}

#[test]
fn deep_control_flow_is_flat_on_success_and_failure() {
    let depth = 12_000;
    let source = format!(
        "{}break{}",
        "while (x) {if (y) {".repeat(depth),
        "}}".repeat(depth)
    );
    let p = parse(&source).unwrap();
    assert_eq!(p.blocks.len(), depth * 2);
    assert_eq!(p, p.clone());
    drop(p);
    assert_eq!(
        parse(&source[..source.len() - 1]).unwrap_err().span.offset,
        source.len() - 1
    );
    let open_frames = "while (x) {if (y) {".repeat(depth);
    let e = parse(&open_frames).unwrap_err();
    assert_eq!(e.code, Code::EXPECTED_SYNTAX);
    assert_eq!(
        e.span,
        Span::new(open_frames.len(), 0, 1, open_frames.len() + 1)
    );
    assert!(e.message.contains("close the if body"));
    let invalid_open_frames = format!("{open_frames}let = 1");
    let e = parse(&invalid_open_frames).unwrap_err();
    assert_eq!(e.code, Code::EXPECTED_SYNTAX);
    assert_eq!(
        e.span,
        Span::new(open_frames.len() + 4, 1, 1, open_frames.len() + 5)
    );
    let chain = format!("if (x) {{}}{} else {{1}}", " else if (y) {}".repeat(depth));
    let p = parse(&chain).unwrap();
    let StatementKind::If { branches, .. } = &p.statements[0].kind else {
        panic!()
    };
    assert_eq!(branches.len(), depth + 1);
    assert_eq!(p, p.clone());
    drop(p);
    let invalid = format!("{chain} else");
    assert_eq!(parse(&invalid).unwrap_err().code, Code::EXPECTED_SYNTAX);
}

#[test]
fn control_statement_separator_preserves_the_following_statement() {
    let p = parse("if (x) {}; x").unwrap();
    assert_eq!(p.statements.len(), 2);
    let StatementKind::If {
        branches,
        else_branch,
    } = &p.statements[0].kind
    else {
        panic!()
    };
    assert_eq!(branches.len(), 1);
    assert!(else_branch.is_none());
    assert_name(&p, branches[0].condition.expression, "x");
    assert!(p.block(branches[0].body).statements.is_empty());
    assert_eq!(p.statements[0].terminator, Some(Span::new(9, 1, 1, 10)));
    assert_eq!(p.statements[0].span, Span::new(0, 10, 1, 1));
    let StatementKind::Expression(id) = p.statements[1].kind else {
        panic!()
    };
    assert_name(&p, id, "x");
    assert_eq!(p.statements[1].span, Span::new(11, 1, 1, 12));
    assert_eq!(p.statements[1].terminator, None);
}

#[test]
fn iteration_keeps_optional_iterable_and_binding_read_separate() {
    let p = parse("for (item in source?.items) {let x = item}").unwrap();
    let StatementKind::For {
        binding,
        iterable,
        body,
        ..
    } = &p.statements[0].kind
    else {
        panic!()
    };
    assert_eq!(binding.name, "item");
    let ExpressionKind::Access { base, links } = &p.expression(*iterable).kind else {
        panic!()
    };
    assert_name(&p, *base, "source");
    assert_eq!(links.len(), 1);
    assert!(links[0].optional);
    let AccessKind::Property(property) = &links[0].kind else {
        panic!()
    };
    assert_eq!(property.name, "items");
    let statements = &p.block(*body).statements;
    assert_eq!(statements.len(), 1);
    let StatementKind::Let {
        name, initializer, ..
    } = &statements[0].kind
    else {
        panic!()
    };
    assert_eq!(name.name, "x");
    assert_name(&p, *initializer, "item");
    assert_ne!(*initializer, *iterable);
    assert_eq!(statements[0].terminator, None);
}

#[test]
fn successful_blocks_keep_bom_crlf_and_unicode_positions() {
    let source = "\u{feff}if (\"ğ\") {\r\n  \"中\"\r\n}";
    let p = parse(source).unwrap();
    let StatementKind::If { branches, .. } = &p.statements[0].kind else {
        panic!()
    };
    let condition = p.expression(branches[0].condition.expression);
    assert_eq!(condition.span, Span::new(7, 4, 1, 5));
    assert_eq!(
        condition.kind,
        ExpressionKind::Literal(Literal::String("ğ".into()))
    );
    let body = p.block(branches[0].body);
    assert_eq!(body.open, Span::new(13, 1, 1, 10));
    assert_eq!(body.close, Span::new(25, 1, 3, 1));
    assert_eq!(body.span, Span::new(13, 13, 1, 10));
    assert_eq!(body.statements[0].span, Span::new(18, 5, 2, 3));
    let StatementKind::Expression(id) = body.statements[0].kind else {
        panic!()
    };
    assert_eq!(
        p.expression(id).kind,
        ExpressionKind::Literal(Literal::String("中".into()))
    );
    assert_eq!(&source[body.span.range()], "{\r\n  \"中\"\r\n}");
}

#[test]
fn incomplete_headers_report_construct_at_eof() {
    for (source, construct) in [
        ("for (x in xs", "for"),
        ("if (x", "if"),
        ("if (x) {} else if (", "else if"),
    ] {
        let e = parse(source).unwrap_err();
        assert_eq!(e.code, Code::EXPECTED_SYNTAX);
        assert_eq!(e.span, Span::new(source.len(), 0, 1, source.len() + 1));
        assert!(e.message.contains(construct), "{}", e.message);
    }
}

#[test]
fn loop_context_does_not_leak_to_sibling_branch_or_after_for() {
    for source in [
        "if (x) {while (y) {}} else {continue}",
        "for (x in xs) {}; continue",
    ] {
        let e = parse(source).unwrap_err();
        let offset = source.find("continue").unwrap();
        assert_eq!(e.code, Code::LOOP_CONTROL_OUTSIDE_LOOP);
        assert_eq!(e.span, Span::new(offset, 8, 1, offset + 1));
        assert!(e.message.contains("continue") && e.message.contains("enclosing loop"));
    }
}

#[test]
fn standalone_blocks_link_siblings_and_nested_declarations_in_order() {
    let source = "{let x = 1; {let x = 2}; let y = 3}; {let x = 4}";
    let p = parse(source).unwrap();
    assert_eq!(p.statements.len(), 2);
    let StatementKind::Block(first) = p.statements[0].kind else {
        panic!()
    };
    let StatementKind::Block(second) = p.statements[1].kind else {
        panic!()
    };
    assert_ne!(first, second);
    let outer = p.block(first);
    assert_eq!(outer.statements.len(), 3);
    let StatementKind::Block(nested) = outer.statements[1].kind else {
        panic!()
    };
    assert_ne!(first, nested);
    assert_ne!(second, nested);
    assert_eq!(
        &source[outer.span.range()],
        "{let x = 1; {let x = 2}; let y = 3}"
    );
    assert_eq!(&source[p.block(nested).span.range()], "{let x = 2}");
    assert_eq!(&source[p.block(second).span.range()], "{let x = 4}");
    for (statement, name, value, text) in [
        (&outer.statements[0], "x", 1.0, "let x = 1;"),
        (&p.block(nested).statements[0], "x", 2.0, "let x = 2"),
        (&outer.statements[2], "y", 3.0, "let y = 3"),
        (&p.block(second).statements[0], "x", 4.0, "let x = 4"),
    ] {
        let StatementKind::Let {
            name: binding,
            initializer,
            ..
        } = &statement.kind
        else {
            panic!()
        };
        assert_eq!(binding.name, name);
        assert_eq!(
            p.expression(*initializer).kind,
            ExpressionKind::Literal(Literal::Number(value))
        );
        let offset = source.find(text).unwrap();
        assert_eq!(statement.span, Span::new(offset, text.len(), 1, offset + 1));
    }
    assert_eq!(
        outer.statements[1].terminator,
        Some(Span::new(23, 1, 1, 24))
    );
}

#[test]
fn grouped_and_indexed_headers_preserve_expression_links_and_delimiters() {
    let source = "if ((a + b) > xs[i]) {}";
    let p = parse(source).unwrap();
    let StatementKind::If { branches, .. } = &p.statements[0].kind else {
        panic!()
    };
    let condition = &branches[0].condition;
    assert_eq!(condition.open, Span::new(3, 1, 1, 4));
    assert_eq!(condition.close, Span::new(19, 1, 1, 20));
    let (left, op, right) = binary(&p, condition.expression);
    assert_eq!(op, BinaryOperator::Greater);
    let ExpressionKind::Group {
        open,
        expression,
        close,
    } = p.expression(left).kind
    else {
        panic!()
    };
    assert_eq!(open, Span::new(4, 1, 1, 5));
    assert_eq!(close, Span::new(10, 1, 1, 11));
    let (a, op, b) = binary(&p, expression);
    assert_eq!(op, BinaryOperator::Add);
    assert_name(&p, a, "a");
    assert_name(&p, b, "b");
    let ExpressionKind::Access { base, links } = &p.expression(right).kind else {
        panic!()
    };
    assert_name(&p, *base, "xs");
    assert_eq!(links.len(), 1);
    let AccessKind::Index {
        open,
        expression,
        close,
    } = links[0].kind
    else {
        panic!()
    };
    assert_eq!(open, Span::new(16, 1, 1, 17));
    assert_eq!(close, Span::new(18, 1, 1, 19));
    assert_name(&p, expression, "i");

    let p = parse("for (x in (xs[i])) {}").unwrap();
    let StatementKind::For {
        open,
        close,
        iterable,
        ..
    } = p.statements[0].kind
    else {
        panic!()
    };
    assert_eq!(open, Span::new(4, 1, 1, 5));
    assert_eq!(close, Span::new(17, 1, 1, 18));
    let ExpressionKind::Group {
        open,
        expression,
        close,
    } = p.expression(iterable).kind
    else {
        panic!()
    };
    assert_eq!(open, Span::new(10, 1, 1, 11));
    assert_eq!(close, Span::new(16, 1, 1, 17));
    let ExpressionKind::Access { base, links } = &p.expression(expression).kind else {
        panic!()
    };
    assert_name(&p, *base, "xs");
    assert_eq!(links.len(), 1);
    let AccessKind::Index {
        open,
        expression,
        close,
    } = links[0].kind
    else {
        panic!()
    };
    assert_eq!(open, Span::new(13, 1, 1, 14));
    assert_eq!(close, Span::new(15, 1, 1, 16));
    assert_name(&p, expression, "i");
}
