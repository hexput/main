# Epic 1 Context: Hexput language core

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

Give script authors a locally runnable Hexput language: source is tokenized, parsed, evaluated, diagnosed, and optionally checked through a CLI without a Daemon, socket, or Backend. The core must be independently testable and fuzzable, and must provide the shared AST and diagnostics that later execution modes and editor tooling consume. Hexput targets short, frequently invoked rules often written by non-engineers, so the governing stance is: conversions a person would predict happen automatically; conversions a person would have to look up are errors.

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

- The language reference is normative; where a story disagrees, the reference wins. Its decision markers are approved choices. Do not borrow JavaScript semantics where Hexput differs. Where the reference's summary tables conflict with its specific rules (a table saying only `bool` is accepted in conditions; a table listing out-of-range index as a reference error), the specific truthiness and absent-data rules below win.
- **Values:** `null`, `bool`, `number` (one IEEE-754 double type, no integer type), `string`, `array` (ordered, heterogeneous, zero-indexed), `object` (string keys, insertion-ordered), and first-class functions. Every result must be finite: division by zero, `NaN`, or infinity raise `arithmetic` errors.
- **Truthiness:** `if`, `while`, `&&`, `||`, `!` accept any value. Falsy is exactly `null`, `false`, `0`, `""`, empty array, empty object; everything else is truthy. `&&`/`||` short-circuit and return an operand (`||`: left if truthy else right; `&&`: left if falsy else right). `!` always returns `bool`.
- **`+`:** if either operand is a string, concatenate using to-string; otherwise to-number both and add (`"10" + 5` → `"105"`, `true + 1` → `2`).
- **`- * / %`:** to-number both operands; a non-numeric string is a `type` error, never `NaN`.
- **Ordering `< <= > >=`:** two strings compare lexicographically by Unicode code point; otherwise to-number both.
- **Equality `== !=`:** same type compares directly, with arrays/objects compared by identity, never structurally. For number vs string, the string is converted to number, and a non-numeric string yields `false` rather than an error. Every other cross-type pair is `false` (`0 == false`, `"" == false`, `[] == false` are all `false`). `null` equals only `null`.
- **To-string:** `null` → `"null"`; bools → `"true"`/`"false"`; numbers use the shortest round-tripping form, with whole values printed without a decimal point (`5`); arrays/objects are a `type` error.
- **To-number:** `null` → 0; bools → 1/0; strings parse only as a valid number literal with optional surrounding whitespace, else a `type` error; arrays/objects are a `type` error. Every mismatch not listed is a `type` error naming both operand types.
- **Access:** reading an absent object key or an out-of-range array index yields `null`; writing an absent key creates it. A non-number index on an array, or a number index on a non-collection, is a `type` error. Ordinary property access on `null` is a `reference` error naming the null link, and so is an undeclared identifier.
- **Optional access:** `a?.b` / `a?.[b]` yield `null` when `a` is `null` and short-circuit the whole remaining chain, which is never evaluated. They suppress only null access, never `type` errors. There is no optional call.
- **Scope:** lexical and block-level; `let` requires an initializer. `let`, named functions, and parameters share one block namespace. Redeclaring a name in the same block is a compile-time error. Inner blocks may shadow, and the outer binding is intact afterward. Assigning to an undeclared name is a runtime `reference` error, with no implicit globals.
- **Functions and control flow:** calls take exactly the declared positional argument count, otherwise an `arity` error with no null padding. Closures capture their defining scope by reference. A missing or bare `return` yields `null`. Top-level `return` is valid and produces the Script result. `for (x in arr)` binds elements; `for (k in obj)` binds string keys in insertion order. Mutating the iterated collection is a runtime error. Recursion is bounded by a call-depth limit that raises `depth`, never a host stack overflow. Scripts cannot catch errors; any error terminates execution.
- **Error categories:** `lexical`, `syntax`, `type`, `reference`, `arity`, `arithmetic`, `depth`, `capability`, `budget`, `policy`. Every error and check finding carries a category, stable code, message, and structured span, with multi-line spans rendered untruncated. Consumers must never parse formatted text.
- Malformed input and failing scripts produce defined errors, never panics. Unsafe Rust in the lexer, parser, and interpreter is forbidden by lint.
- **No ambient host access:** there is no import, filesystem, network, environment, process, or standard library (not even `len`/`push`). The host is reached only through Registered Function calls, and an unknown name is a `capability` error. Don't add a second host path for local convenience.
- **Static check:** it never executes code, infers types across bindings, or replaces runtime enforcement. Unused locals are warnings only. A missing callable-name list suppresses the unknown-call finding, which is different from an empty list. The CLI check fails only on error-severity findings and distinguishes "clean" from "warnings only".

## Technical Decisions

- Crate boundaries are compiler-enforced. `hexput-interpreter` depends on `hexput-ast` only, with no lexer/parser, host, or daemon reach. `hexput-check` depends on AST plus shared types, never on the interpreter, RPC, or enforce. `hexput-cli-core` composes lexer, parser, interpreter, and check. `hexput-bin` holds thin `main`s only. Adding an edge means updating the crate-graph check script in the same change.
- The shared diagnostic shape lives in `hexput-shared::diagnostics`; do not create parallel error formats per subsystem.
- The evaluator must stay stack-safe on deep input, consistent with the parser's continuation-driven design. The call-depth limit is an explicit counter, not reliance on the native stack.
- Budget and Capability enforcement are not the interpreter's job. They arrive later through the single Executor, so design evaluation to be driven or hooked from outside rather than embedding policy.
- Toolchain: Rust 1.98.1, edition 2024; external versions pinned once at workspace level.

## Cross-Story Dependencies

- 1.6 consumes the AST from 1.3–1.5 and establishes values, environments, and runtime errors that 1.7 extends with control flow, calls, closures, and depth limits. Runtime errors must use the shared diagnostic shape so that 1.8 renders them without rework.
- 1.9 composes parser, interpreter, and diagnostics, and binds CLI-supplied starting variables. 1.10 adds the pure checker and a CLI check command.
- Later epics reuse this core: Direct/Cached Execution (Epics 2, 4), check-mode and language-feature policy from Backend Config (Epic 3), Plugin syntax and Global Variables (Epic 6: `plugin`, `@Event`, `@Global`; functions are not storable in Global Variables), and the grammar and LSP, which use parser and check only (Epic 9). Keep Plugin-specific behavior out of Epic 1.
