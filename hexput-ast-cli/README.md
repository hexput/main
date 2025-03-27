# AST Resolver Core

A command-line tool that resolves code into an Abstract Syntax Tree (AST). This tool is designed to parse a custom scripting language and output a JSON representation of the AST.

## Installation

```bash
cargo build --release
```

## Usage

```bash
# Basic usage
cargo run -p hexput-ast-cli -r -- "vl x = 5; print(x);"

# Use :: separator for the code
cargo run -p hexput-ast-cli -r -- :: "vl x = 5; print(x);"

# With feature flags
cargo run -p hexput-ast-cli -r -- --no-loops --no-callbacks :: "vl x = 5; print(x);"
```

## Language Features

The language supports these main features:

### Variable Declarations

```
vl name = "John";
vl age = 30;
```

### Conditional Statements

```
if name == "John" {
  print("Hello John!");
}
```

### Loops

```
loop item in items {
  print(item);
}
```

### Loop Control

```
loop item in items {
  if item == "stop" {
    end;
  }
  if item == "skip" {
    continue;
  }
  print(item);
}
```

### Callbacks (Functions)

```
cb add(a, b) {
  res a + b;
}

vl result = add(5, 3);
```

### Objects

```
vl person = {
  name: "John",
  age: 30,
  address: {
    city: "New York"
  }
};
```

### Arrays

```
vl numbers = [1, 2, 3, 4];
vl nested = [[1, 2], [3, 4]];
```

### Object and Array Navigation

```
vl name = person.name;
vl first = numbers[0];
vl city = person["address"]["city"];
```

### Dynamic Property Access

```
vl key = "name";
vl value = person[key];
```

### Object Keys

```
vl keys = keysof person;
loop key in keysof person {
  print(key, person[key]);
}
```

## Feature Flags

You can disable specific language features using command-line flags:

| Flag | Description |
|------|-------------|
| `--no-object-constructions` | Disable object literal construction `{ key: value }` |
| `--no-array-constructions` | Disable array literal construction `[1, 2, 3]` |
| `--no-object-navigation` | Disable object property access (`obj.prop` or `obj["prop"]`) |
| `--no-variable-declaration` | Disable variable declarations with `vl` |
| `--no-loops` | Disable loop statements |
| `--no-object-keys` | Disable `keysof` operator |
| `--no-callbacks` | Disable callback declarations |
| `--no-conditionals` | Disable if statements |
| `--no-return-statements` | Disable return statements with `res` |
| `--no-loop-control` | Disable loop control statements (`end`, `continue`) |
| `--no-equality` | Disable equality operator (`==`) |
| `--no-assignments` | Disable assignment operator (`=`) |

When a feature is disabled, the parser will skip and ignore those constructs rather than throwing an error.

## Output Options

| Flag | Description |
|------|-------------|
| `--minify` | Output minified JSON without whitespace |
| `--no-source-mapping` | Remove source location information from the output JSON |

## AST Structure

The output is a JSON representation of the Abstract Syntax Tree. The AST has these main components:

- `Program`: The root of the AST, containing a list of statements
- `Statement`: Represents individual instructions like variable declarations, if statements, etc.
- `Expression`: Represents values, operations, and computations
- `Block`: A group of statements enclosed in braces

Each node in the AST has a `type` property that identifies its kind.

## Examples

### Basic Example

```bash
cargo run -p hexput-ast-cli -r -- --no-source-mapping :: "vl x = 5; vl y = x + 3; print(x, y);"
```

### Disable Specific Features

```bash
cargo run -p hexput-ast-cli -r -- --no-variable-declaration :: "vl x = 5; vl y = x + 3; print(x, y);"
```

This will throw error at the variable declarations.

## Error Handling

When a syntax error is encountered, the tool will output a JSON error with a message explaining the issue.
