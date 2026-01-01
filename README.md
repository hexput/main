# Hexput Language Runtime

A deterministic, synchronous, sandboxed RPC language implemented in Rust.

## Overview

Hexput is a language designed for secure, isolated script execution with explicit RPC semantics. Scripts execute in sandboxed contexts with capability-based security and enforced resource limits.

## Architecture

The runtime is organized into strict layers:

- **language**: Lexical and syntactic analysis (lexer, parser, AST)
- **semantic**: Name resolution, symbol tables, capability checking
- **runtime**: VM, execution contexts, value representation
- **rpc**: Protocol definitions, message dispatching, registry
- **transport**: WebSocket, Unix domain sockets, named pipes
- **sandbox**: Resource limits, guards, timeouts
- **util**: Shared utilities

## Transport Options

Hexput supports three transport mechanisms for RPC:

1. **WebSocket** - Remote connections, cross-platform
2. **Unix Domain Sockets** (Linux/macOS) - High-performance local IPC
3. **Named Pipes** (Windows) - Native Windows IPC and service integration

See [docs/TRANSPORT.md](docs/TRANSPORT.md) for detailed transport documentation.

## Execution Modes

Hexput supports two execution modes:

1. **Direct Execution** (`ExecutionStart`) - Parse and execute immediately
2. **Cached Execution** (`CodeRegister` + `CachedExecutionStart`) - Parse once, execute many times

Cached execution provides significant performance benefits for repeated script execution with different variables.

See [docs/CACHED_EXECUTION.md](docs/CACHED_EXECUTION.md) for details.

## Security Model

**Function Registration Required**: Remote functions and methods must be explicitly registered before scripts can call them.

This provides:
- **Security**: Prevents unauthorized function access
- **Performance**: Blocks abuse (e.g., random function names in loops)
- **Control**: Explicit allow-list per context

See [docs/FUNCTION_REGISTRATION.md](docs/FUNCTION_REGISTRATION.md) for details.

## Language Syntax

### Variables
```hexput
vl x = 42;
vl name = "Alice";
vl config = { timeout: 5000, enabled: true };
```

### Callbacks (Functions)
```hexput
cb add(a, b) {
  res a + b;
}

vl result = add(10, 20);
```

### Loops
```hexput
vl items = [1, 2, 3, 4, 5];
loop item in items {
  if item == 3 {
    continue;
  }
  // Process item
}
```

### Conditionals
```hexput
if x == 42 {
  res "found";
}
```

### Objects and Arrays
```hexput
vl person = {
  name: "Alice",
  age: 30,
  active: true
};

vl keys = keysof person;
vl nameValue = person.name;
```

## Key Features

### Deterministic Execution
- Single-threaded execution within a context
- Reproducible results for the same inputs
- No race conditions or timing issues

### Capability-Based Security
- Explicit permissions for all operations
- No filesystem access
- No raw socket access
- No host memory access

### Resource Limits
- Maximum instruction count
- Maximum recursion depth
- Execution timeout
- Memory limits (future)

### RPC Integration
- Blocking semantics from script perspective
- Async transport internally
- JSON-based message protocol
- Function registry for available calls

## Building

### Standard Build
```bash
cargo build --release
```

This builds the normal `hexput` executable.

### Windows Service Build
On Windows, to build both executables:
```bash
# Normal executable
cargo build --release

# Windows service executable
cargo build --release --features windows-service --bin hexput-service
```

This creates:
- `target/release/hexput.exe` - Standard command-line tool
- `target/release/hexput-service.exe` - Windows service executable

### Installing Windows Service
```powershell
# Run as Administrator
sc.exe create hexput binPath= "C:\path\to\hexput-service.exe" start= auto
sc.exe start hexput
```

## Running

```bash
cargo run
```

## Testing

```bash
cargo test
```

## Examples

See the `examples/` directory for sample scripts and usage patterns.

## Development Status

This is an early-stage project. Current status:

- ✅ Lexer and parser
- ✅ Basic AST representation
- ✅ Symbol table and name resolution
- ✅ VM with local execution
- ✅ Capability system
- ✅ Resource limits
- ✅ RPC protocol definitions
- ✅ WebSocket transport
- ✅ Unix domain socket transport (Linux/macOS)
- ✅ Named pipe transport (Windows)
- ✅ Windows service support
- ⚠️  Remote function calls (in progress)
- ⚠️  Complex assignment targets (in progress)
- 🔲 Bytecode optimization
- 🔲 Advanced diagnostics
- 🔲 Debugger support
- ⚠️  Remote function calls (in progress)
- ⚠️  Complex assignment targets (in progress)
- 🔲 Bytecode optimization
- 🔲 Advanced diagnostics
- 🔲 Debugger support

## License

MIT OR Apache-2.0
