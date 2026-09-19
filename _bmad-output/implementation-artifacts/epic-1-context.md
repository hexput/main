# Epic 1 Context: Hexput language core

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

Give script authors a locally runnable Hexput language: source is tokenized, parsed, evaluated, diagnosed, and optionally checked through a CLI without a Daemon, socket, or Backend. The language core must be independently testable and suitable for fuzzing, while providing the shared AST and diagnostics that later execution modes and editor tooling consume. Hexput targets short, frequently invoked rules, including rules written by non-engineers; predictable behavior and clear failures matter more than general-purpose language breadth.

## Stories

- Story 1.1: Project scaffold and pinned toolchain
- Story 1.2: Tokenize Hexput source
- Story 1.3: Parse expressions, declarations, and member access
- Story 1.4: Parse conditionals and loops
- Story 1.5: Parse functions, callbacks, objects, and arrays
- Story 1.6: Evaluate expressions and variable scope
- Story 1.7: Execute control flow, functions, and callbacks
- Story 1.8: Report errors with precise source locations
- Story 1.9: Evaluate a script from the command line
- Story 1.10: Check a script without running it

## Requirements & Constraints

- The language reference is normative when story wording conflicts with it. Its decision markers record approved choices, not unresolved questions. Do not borrow JavaScript semantics where Hexput specifies different behavior.
- UTF-8 source supports Unicode string content but only ASCII identifiers. Semicolons separate statements; the last statement in a block or file may omit its terminator. String literals can span lines. Reserved words cannot serve as identifiers, bare object keys, or parameter names.
- Preserve precise source locations throughout tokens, AST nodes, and failures. Errors and findings share a machine-readable category, stable code, message, and structured span; terminal rendering must retain multiline location information and expose the offending source. Editor clients must not need to parse formatted text.
- Malformed input and script failures must produce defined errors rather than panics or host stack overflow. Recursion is permitted but bounded. Non-finite numeric results and division by zero fail explicitly. Memory safety in the parser and evaluator is a security property; unsafe Rust requires explicit review and a justified safety explanation.
- The language is the permanent trust boundary. There are no ambient filesystem, network, environment, process, module, or import facilities, and no built-in standard library. Host interaction later uses Registered Functions exclusively; do not add a second host-access path to make local evaluation convenient.
- Static checking never executes code, infers types across bindings, or substitutes for runtime Capability and Resource Budget enforcement. It checks structurally decidable mistakes using caller-supplied names and policy; unused locals remain warnings. CLI checking returns failure only for error-severity findings.
- The standalone CLI accepts starting variables, prints successful results, and renders failures to stderr with a nonzero exit code. A successful check and a check with warnings remain distinguishable.

## Technical Decisions

- Keep AST data, lexing, parsing, evaluation, checking, and CLI behavior behind separate crate boundaries. AST contains data rather than parsing or evaluation logic. The interpreter is a tree-walking evaluator without host reach. The only binary-producing crate contains thin entry points that hand off to libraries.
- Respect the prescribed dependency graph: AST and lexer may use shared types; parser depends on lexer and AST; interpreter depends on AST; checker depends on AST and shared types, never interpreter, RPC, or enforcement. CLI composes the language libraries. Shared diagnostics define the common error shape instead of parallel subsystem-specific formats.
- Use the pinned Rust 1.98.1 toolchain and 2024 edition. Centralize external dependency versions at workspace level. Architecture boundaries must remain enforceable as dependencies, not just naming conventions.
- Values include null, booleans, finite double-precision numbers, strings, ordered arrays, insertion-ordered objects, and first-class functions. Functions capture lexical environments by reference. Local bindings are block scoped; inner blocks may shadow outer ones, but duplicate declarations within a block are syntax errors.
- Preserve operator precedence and left associativity of binary operators. Calls and access bind tighter than unary operators, which bind tighter than binary operators. Conditions use truthiness, including falsy empty collections; logical operators short-circuit and return an operand. Numeric conversions, concatenation, and equality follow the language's explicit conversion rules rather than broad coercion.
- Missing object keys and out-of-range array reads yield null; undeclared identifiers and ordinary access on null remain reference errors. Optional property/index access suppresses null access only and short-circuits the remaining chain. There is no optional-call syntax.
- The checker exposes one pure, stateless entry point over an AST, callable-name information, and policy. A missing callable-name list differs from an explicitly empty list. Later submission paths invoke checking once before execution or registration, never on every Cached Execution.

## Cross-Story Dependencies

- Scaffold precedes lexing; expression parsing establishes the AST extended by control-flow and function/collection parsing. Evaluation consumes that shared representation. Diagnostic structure must remain compatible throughout, even before terminal rendering is completed.
- CLI evaluation composes the parser, evaluator, and diagnostics; CLI checking adds the pure checker. Keep these usable independently of daemon infrastructure.
- Later epics reuse this core for Direct Execution, AST Cache reuse, and Plugin execution. Plugin-specific syntax and persistent Global Variables belong to Epic 6; do not expand ordinary script work into Plugin registration or lifecycle behavior.
- Epic 3 supplies the Backend Config modes and per-execution overrides for checking and language-feature policy. Epic 9 consumes parser/checker diagnostics without depending on the interpreter or daemon. Preserve reusable structured outputs for these consumers.
