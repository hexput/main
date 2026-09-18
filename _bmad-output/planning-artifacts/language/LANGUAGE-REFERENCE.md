---
title: Hexput Language Reference (v2)
status: final
created: 2026-09-18
author: drafted by PM role during the sprint-planning readiness gate; approved by Erdem 2026-09-18 after four rounds of revision (@Global syntax, truthiness + implicit conversion, absent-key null, optional chaining + static check)
note: '[DECISION] markers are retained as provenance — they mark choices made here rather than inherited from the PRD, brief, or spine. They are approved, not open.'
purpose: Close readiness-gate finding A — Epic 1 (lexer/parser/interpreter), Epic 6 (plugin syntax) and Epic 9 (tree-sitter grammar + LSP) all depend on a language definition that no planning artifact recorded.
---

# Hexput Language Reference — v2

This is the normative definition of the Hexput language. Epic 1 implements it, Epic 9's grammar and language server describe it, and Epic 6's plugin parsing extends it. Where this document and a story disagree, this document wins and the story is corrected.

Every item marked **[DECISION]** is a choice made here for the first time, not inherited from the PRD, brief, or architecture spine. Each is defensible but reversible — they are exactly what the readiness gate flagged as "decisions nothing records".

## 1. Design stance

Hexput serves two audiences at once and the language leans toward the second: the backend engineer running the daemon, and the **non-engineer author writing the actual rule** (PRD §2, UJ-2's academic-affairs staffer). The language is therefore forgiving where forgiveness is guessable — conditions accept any value, `+` builds strings out of whatever you give it — and strict only where guessing would produce a confidently wrong answer.

The line between the two: **conversions that a person would predict are automatic; conversions that a person would have to look up are errors.** `"Total: " + 5` is obvious, so it works. `"abc" * 2` is not, so it fails. `0.1 + 0.2` is honest arithmetic. Dividing by zero is not a number, so it raises rather than yielding infinity.

The language is small on purpose — it targets short, frequently-invoked rules, not general programming.

## 2. Lexical structure

- **Encoding:** UTF-8 source. Identifiers are ASCII letters, digits, and `_`, not starting with a digit. **[DECISION]** Non-ASCII identifiers are rejected; non-ASCII content in string literals is fully supported.
- **Comments:** `//` to end of line, and `/* ... */` which does not nest. **[DECISION]**
- **Whitespace** is insignificant except as a token separator.
- **Statement termination:** statements are terminated by `;`. **[DECISION]** The terminator may be omitted for the last statement in a block or file — this accommodates the PRD §4.9 example, which omits it after the `let b = {...}` declaration.
- **Reserved words:** `let`, `fn`, `if`, `else`, `while`, `for`, `in`, `return`, `break`, `continue`, `true`, `false`, `null`, `plugin`. **[DECISION]** Reserved words may not be used as identifiers, object keys written bare, or parameter names.

## 3. Values and types

Hexput is dynamically typed with six value types:

| Type | Literal form | Notes |
| --- | --- | --- |
| `null` | `null` | The only value of its type; not a default for anything |
| `bool` | `true`, `false` | The only type accepted in a condition |
| `number` | `1`, `-3`, `2.5`, `1e3` | **[DECISION]** One numeric type, IEEE-754 double. No separate integer type |
| `string` | `"text"`, `'text'` | **[DECISION]** Escapes `\n \t \r \\ \" \' \u{...}`. No interpolation in v2. **[DECISION]** String literals may span lines: a raw newline inside one is ordinary content, not an error |
| `array` | `[1, 2, 3]` | Ordered, heterogeneous, zero-indexed; trailing comma allowed |
| `object` | `{ key: "value" }` | String keys, insertion-ordered; bare or quoted keys; trailing comma allowed. Reading an absent key yields `null` (§7) |

**[DECISION] String literals are multi-line.** A raw newline between the quotes is kept verbatim in the value, so

```
let a = "merhaba
sosis
ben";
```

is one string containing two newlines. Consequently a string is unterminated only at end of input, never at end of line, and a string's source span may cover several lines (Story 1.8 renders multi-line spans without truncating the location). There is no line-continuation escape: a `\` immediately before a newline is an invalid escape, not a join.

Functions are values (§6) but are not storable in a Global Variable. **[DECISION]** — a function closes over an environment, and persisting one across Event invocations would make Global Variable lifetime semantics (FR-25) undefinable.

**[DECISION] Number edge cases:** division by zero, and any operation producing `NaN` or infinity, raise a runtime error rather than yielding a non-finite value. A rules engine that returns `NaN` has failed, not computed.

## 4. Operators

Precedence, tightest first. All binary operators are left-associative; unary operators bind tighter than any binary operator.

| Level | Operators | Meaning |
| --- | --- | --- |
| 1 | `a.b` `a[b]` `a?.b` `a?.[b]` `f(...)` | Member access, index, optional access, call |
| 2 | `-a` `!a` | Numeric negation, boolean negation |
| 3 | `*` `/` `%` | Multiplication, division, remainder |
| 4 | `+` `-` | Addition/concatenation, subtraction |
| 5 | `<` `<=` `>` `>=` | Ordering comparison |
| 6 | `==` `!=` | Equality |
| 7 | `&&` | Logical and, short-circuiting |
| 8 | `\|\|` | Logical or, short-circuiting |

### 4.1 Truthiness

**[DECISION]** `if`, `while`, `&&`, `||`, and `!` accept **any** value. A value is **falsy** when it is one of exactly these five, and truthy otherwise:

`null` · `false` · `0` · `""` (empty string) · an empty array or empty object

**[DECISION]** Empty collections are falsy, following Python rather than JavaScript — `if (items)` reading as "if there are any items" is what a rule author expects, and JavaScript's truthy `[]` is a common source of silently wrong conditions.

`&&` and `||` **return one of their operands**, not a `bool`, so the useful idioms work: `let name = input.name || "unknown";` yields `"unknown"` only when `input.name` is falsy. `||` returns the left operand when it is truthy, otherwise the right; `&&` returns the left when it is falsy, otherwise the right. Short-circuiting applies — the right operand is not evaluated when the left decides the result. `!` accepts any value and always returns `bool`.

### 4.2 Implicit conversion

Conversions happen automatically in the cases below. **[DECISION]** Every other type mismatch is a `type` error naming both operand types.

**`+` — addition or concatenation.** If **either** operand is a `string`, the result is string concatenation and the other operand is converted with the to-string rules (§4.3). Otherwise both operands are converted with the to-number rules and added.

```
"Total: " + 5      // "Total: 5"
"Order " + null    // "Order null"
"10" + 5           // "105"      — string wins
true + 1           // 2          — no string involved
```

**`-` `*` `/` `%` — arithmetic.** Both operands are converted with the to-number rules. A string that does not look like a number is a `type` error, not a silent `NaN`.

```
"10" - 1     // 9
true * 3     // 3
"abc" * 2    // type error
```

**`<` `<=` `>` `>=` — ordering.** If **both** operands are `string`, they compare lexicographically by Unicode code point. Otherwise both are converted to number.

**`==` and `!=` — equality.** **[DECISION]** Deliberately narrower than JavaScript's, because JavaScript's is the part everyone gets wrong:

- Same type → compared directly. Arrays and objects compare **by identity**, never structurally.
- `number` vs `string` → the string is converted to number; if it does not look like a number, the result is `false` rather than an error.
- Every other cross-type comparison → `false`. So `0 == false` is `false`, `"" == false` is `false`, and `[] == false` is `false` — none of JavaScript's famous surprises apply.
- `null == null` is `true`; `null` equals nothing else.

Truthiness (§4.1) and equality are deliberately different questions: `0` is falsy but `0 == false` is `false`.

### 4.3 Conversion rules

| To string | Result |
| --- | --- |
| `null` | `"null"` |
| `bool` | `"true"` / `"false"` |
| `number` | Shortest representation that round-trips; whole values print without a decimal point (`5`, not `5.0`) |
| `array` / `object` | **[DECISION]** `type` error — no `"[object Object]"`. Formatting a collection is the host's job via a Registered Function |

| To number | Result |
| --- | --- |
| `null` | `0` |
| `bool` | `1` / `0` |
| `string` | Parsed if it is a valid number literal with optional surrounding whitespace; otherwise a `type` error |
| `array` / `object` | `type` error |

### 4.4 Optional access

**[DECISION]** `?.` reads a property or index without raising when the left side is `null`:

```
order.customer?.name        // null when customer is null, instead of a reference error
order?.items?.[0]           // null when order or items is null
```

- `a?.b` and `a?.[b]` evaluate to `null` when `a` is `null`, and otherwise behave exactly like `a.b` and `a[b]`.
- **[DECISION] Short-circuiting covers the rest of the chain**, as in JavaScript: in `a?.b.c.d`, if `a` is `null` the whole expression is `null` and `b.c.d` is never evaluated — so one `?.` at the uncertain link is enough, rather than one at every link.
- `?.` is only about `null`. It does not suppress `type` errors, and it is not a general error-swallowing operator.
- **[DECISION]** There is no optional call form (`a?.()`); a Registered Function either exists or the call is a `capability` error, which `?.` must not hide.

## 5. Statements

```
let x = <expr>;              // declaration; initializer required [DECISION]
x = <expr>;                  // assignment to a declared binding
obj.key = <expr>;            // member assignment
arr[0] = <expr>;             // index assignment

if (<expr>) { ... } else if (<expr>) { ... } else { ... }   // any value; §4.1 truthiness

while (<expr>) { ... }                                       // any value; §4.1 truthiness

for (item in <array or object>) { ... }

return <expr>;               // expression optional; bare `return` yields null
break;                       // innermost loop only
continue;                    // innermost loop only
```

- **Scoping** is lexical and block-level. A `let` binds in its enclosing block; an inner block may shadow an outer binding, and the outer binding is intact after the block ends.
- **[DECISION]** Re-declaring the same name in the same block is a compile-time error. Assignment to an undeclared name is a runtime error — there is no implicit global creation.
- **[DECISION]** `for (item in array)` binds each element; `for (key in object)` binds each key as a `string`, in insertion order. Mutating the collection being iterated is a runtime error rather than undefined behavior.
- **[DECISION]** `break` and `continue` outside a loop are compile-time errors.

## 6. Functions and callbacks

```
fn name(a, b) { return a + b; }        // named declaration, statement position
let f = fn(a) { return a * 2; };       // anonymous function, expression position
items.each(fn(item) { ... });          // callback as an argument
```

- Parameters are positional. **[DECISION]** Calling with the wrong argument count is a runtime error — no implicit `null` padding and no variadic collection.
- A function body that reaches its end without `return` yields `null`.
- **[DECISION]** Closures capture their defining scope **by reference**, so a callback sees later mutations of a captured binding.
- **[DECISION]** Recursion is permitted and bounded by a call-depth limit; exceeding it is a runtime error (never a host stack overflow — Story 1.7).
- Functions are first-class values: passable, returnable, storable in local bindings, arrays, and objects — but not in Global Variables (§3).

## 7. Errors

Every failure carries a category, a stable code, a message, and a source span (Epic 1 Story 1.8). Categories:

| Category | Raised when | Detected |
| --- | --- | --- |
| `lexical` | Unterminated string, unknown character | Lex time |
| `syntax` | Malformed construct, `break` outside a loop, duplicate `let` | Parse time |
| `type` | A conversion §4.2 does not perform — arithmetic on a non-numeric string, stringifying a collection, a collection in a numeric operand | Runtime |
| `reference` | Undeclared identifier, index out of range, property access on `null` | Runtime |
| `arity` | Wrong argument count | Runtime |
| `arithmetic` | Division by zero, non-finite result | Runtime |
| `depth` | Call-depth limit exceeded | Runtime |
| `capability` | Call to an unregistered or denied Registered Function (FR-6, FR-7) | Runtime |
| `budget` | A Resource Budget dimension exceeded (FR-8) | Runtime |
| `policy` | A disabled language construct was used (FR-3) | Parse or runtime |

**[DECISION] Reading a missing object key yields `null`, not an error** — optional fields are the common case for a rule author, and `if (input.discount)` should read as "if a discount was supplied" rather than blowing up. Writing to a missing key creates it.

**[DECISION] Reading an index outside an array's range yields `null`** as well, for the same reason. A non-number index on an array, or a number index on a non-collection, is still a `type` error.

**[DECISION] Two cases stay `reference` errors**, because each means the script is wrong rather than the data being absent:

- **Property access on `null` without `?.`** — `order.customer.name` where `customer` is `null` raises, naming `customer`. Use `order.customer?.name` to opt into `null` instead (§4.4). Without this default, one absent field would silently produce `null` three levels later and the rule would compute a confidently wrong answer while looking like it worked.
- **Undeclared identifier** — a typo'd variable name is never data. The static check (§11) turns this into a pre-execution error when it is enabled.

**[DECISION]** Scripts cannot catch errors in v2 — there is no `try`/`catch`. Any error terminates the execution and is reported to the Backend. Host-side errors from a Registered Function (§8) reach the script the same way and are equally uncatchable.

## 8. Host interaction

A script reaches the host only by calling a Registered Function by its registered name, as an ordinary call expression (FR-6, FR-7):

```
let order = getOrder(orderId);
applyDiscount(order.id, 10);
```

There is no import, require, module, filesystem, network, environment, or process facility in the grammar at all — the absence is structural, not a runtime check (Epic 3 Story 3.4). A call to an unregistered name raises `capability`, indistinguishable from a denied call.

## 9. Plugin source

A Plugin (FR-17) is Hexput source with three additions to the grammar above.

```
plugin {
  name = "loyalty_rules",
  version = "1.0.0",
}

let counter = 0;

@Global(behavior = "ttl", ttl = "5m", locking = "safe")
let cache = {};

@Event(BackendRegisteredInit)
fn setup(params) { counter = 0; }

@Event(OrderPlaced, priority = 1)
fn on_order(params) { counter = counter + 1; return { ok: true }; }

@Event(OrderPlaced, async = true)
fn audit(params) { logOrder(params.id); return { ok: true }; }
```

- **`plugin { }` block** — required, exactly one, first non-comment construct in the file. Keys are bare identifiers assigned literal values with `=`, comma-separated, trailing comma allowed. `name` is required; all other keys are Backend-defined and opaque to the daemon (FR-17).
- **Top-level `let`** declarations are Global Variables (FR-20). **[DECISION]** Their initializer must be a literal or an expression over literals — it may not call a function, because init order across Global Variables would otherwise be observable.
- **`@Global(...)`** — **[DECISION] new in this document; FR-20 and FR-25 require per-variable locking and behavior overrides from plugin code but no syntax was ever specified.** Optional annotation on a top-level `let`, accepting `behavior` (`"forever"` default, `"ttl"`, `"separate_each_trigger"`, `"keyed"`), `ttl` (duration string, required when behavior is `"ttl"`), and `locking` (`"safe"` default, `"unsafe"`). An annotation the Backend's Config does not permit is rejected at registration (FR-20).
- **`@Event(<Name>)`** — binds a function as a handler. Optional `priority = <integer>` (ascending, lower first) and `async = <bool>` (default `false`). `async` wins when both are present and `priority` is then ignored (FR-22, FR-23, OQ-11).
- **[DECISION]** `@Event` and `@Global` may only annotate a top-level `fn` and a top-level `let` respectively; anywhere else is a `syntax` error. A handler takes exactly one parameter, `params`.

## 10. Static check (optional, Backend-configured)

**[DECISION] New capability, recorded here first.** Hexput can run a static check pass over a parsed AST **before** executing it, catching whole classes of mistake at submission time instead of mid-rule. The check is **never mandatory**: the Backend decides per Session whether it runs, and can override that per execution, exactly like any other execution-policy setting (FR-3).

**Modes.** `off` (default — parse straight to execution), `warn` (findings reported alongside a normal execution), `error` (any finding rejects the script before a single statement runs).

**What it checks.** Everything below is decidable without running the script:

| Finding | Why it is checkable |
| --- | --- |
| Undeclared identifier read, or assignment to one | Lexical scoping is fully known from the AST |
| Duplicate `let` in one block; `break`/`continue` outside a loop | Already `syntax` errors — the check reports them with the others |
| Wrong argument count calling a function declared in the same script | Arity is known |
| Call to a name that is neither a local function nor a Registered Function on this Session | The daemon knows the Session's registrations (FR-6) — this catches a typo'd host call before it becomes a `capability` error at runtime |
| Type errors between literal operands — `"abc" * 2`, an array in a numeric operand | No runtime information needed |
| Unreachable code after `return`, `break`, or `continue` | Control flow is structural |
| A disabled language construct (FR-3 toggles) appearing anywhere in the script | The toggle set is known before execution |
| Unused local variable | Reported as a warning only, never an error |
| Plugin-only: `@Event` naming an Event the Backend never declared; `@Global` using a strategy the Backend forbids | Declarations arrive with the registration (FR-19, FR-20) |

**What it deliberately does not do.** It is not a type system: it never infers types across bindings, never checks a handler's return value against its declared shape (that stays the runtime check in FR-19), and never rejects a script for a mistake that depends on runtime values.

**Where the findings go.** Each finding carries the same category, code, message, and source span as any other error (§7), so the CLI, the Backend's error response, and the language server (FR-15) all render them identically — the check is the main reason the language server can offer more than syntax diagnostics.

## 11. Deliberately absent

Not in v2, and not an oversight: `try`/`catch`, `throw`, modules and imports, classes and inheritance, `this`, string interpolation, regular expressions, integer/float distinction, bitwise operators, ternary `?:`, switch, labeled break, generators, `async`/`await` inside scripts (concurrency is the daemon's concern, not the script's), and any standard library beyond what the host registers.

**[DECISION]** There is no built-in standard library at all in v2 — not even `len()` or `push()`. Everything a script can do beyond the operators above comes from Registered Functions. This keeps the trust boundary exactly at the capability edge (NFR1) and is the single most likely item to need revisiting once real scripts are written.
