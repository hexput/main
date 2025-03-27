# Hexput Runtime

A WebSocket server for parsing and executing Hexput AST code with configurable security constraints.

## Overview

Hexput Runtime is a Rust-based execution environment that allows clients to send code via WebSocket connections and receive execution results. The runtime provides:

- Code parsing to an AST representation
- Secure code execution with configurable constraints
- Built-in methods for common data types
- Function call bridging between the runtime and client

## Installation

### Prerequisites

- Rust and Cargo (1.56.0 or later)
- Dependencies are managed through Cargo

### Building from source

```bash
# Clone the repository
git clone https://github.com/hexput/main hexput-main
cd hexput-main/hexput-runtime

# Build the project
cargo build -r

# Run the server
../target/release/hexput-runtime
```

## Usage

```bash
# Run with default settings (127.0.0.1:9001)
./hexput-runtime

# Specify address and port
./hexput-runtime --address 0.0.0.0 --port 9001

# Enable debug logging
./hexput-runtime --debug

# Set specific log level
./hexput-runtime --log-level debug
```

## WebSocket API

### Connecting to the Server

To connect to the Hexput Runtime server, use a WebSocket client to connect to the server's address and port. When the connection is established, the server will send a welcome message:

```json
{"type":"connection","status":"connected"}
```

### Handling WebSocket Connections Properly

For reliable WebSocket communication:

1. **Connection Establishment**:
   - Connect to the server using the WebSocket protocol
   - Wait for the welcome message before sending requests
   - Handle connection failures gracefully with reconnection logic

2. **Message Handling**:
   - Always include a unique ID with each request
   - Process incoming messages asynchronously
   - Keep track of pending requests and their corresponding responses

3. **Connection Management**:
   - Implement ping/pong heartbeats to detect disconnections
   - Gracefully close connections when they're no longer needed
   - Handle reconnection with exponential backoff

4. **Error Handling**:
   - Listen for error messages from the server
   - Handle execution errors by correlating them with the original request ID
   - Implement timeout mechanisms for requests that take too long

### Message Formats

#### Requests (Client -> Server)

The server accepts the following request types:

1. **Parse Request**:
```json
{
  "id": "unique-request-id",
  "action": "parse",
  "code": "vl x = 10;",
  "options": {
    "minify": true,
    "include_source_mapping": false,
    "no_object_constructions": false
  }
}
```

2. **Execute Request**:
```json
{
  "id": "unique-request-id",
  "action": "execute",
  "code": "vl x = 10; return x * 2;",
  "options": {
    "no_loops": true,
    "no_callbacks": true
  },
  "context": {
    "initialValue": 5
  },
  "secret_context": { // Optional: Data passed only to remote functions
    "apiKey": "sensitive-key-123" 
  }
}
```

#### Responses (Server -> Client)

1. **Parse Response**:
```json
{
  "id": "unique-request-id",
  "success": true,
  "result": { /* AST representation */ }
}
```

2. **Execute Response**:
```json
{
  "id": "unique-request-id",
  "success": true,
  "result": { /* Execution result */ }
}
```

3. **Error Response**:
```json
{
  "id": "unique-request-id",
  "success": false,
  "error": "Error message with details"
}
```

#### Remote Function Protocol (Bidirectional)

1.  **Function Existence Check (Server -> Client)**: When the runtime needs to call a function not defined locally.
    ```json
    {
      "id": "check-uuid",
      "action": "is_function_exists",
      "function_name": "calculateTotal"
    }
    ```

2.  **Function Existence Response (Client -> Server)**: Client confirms if it handles the function.
    ```json
    {
      "id": "check-uuid",
      "exists": true
    }
    ```

3.  **Function Call Request (Server -> Client)**: If the function exists, the server requests its execution.
    ```json
    {
      "id": "call-uuid",
      "function_name": "calculateTotal",
      "arguments": [10, 20, {"tax": 0.05}],
      "secret_context": { "apiKey": "sensitive-key-123" } // Included if provided in original execute request
    }
    ```

4.  **Function Call Response (Client -> Server)**: Client returns the result of the function execution.
    ```json
    {
      "id": "call-uuid",
      "result": { /* Function result */ },
      "error": null /* or error message */
    }
    ```

### Secret Context

The `execute` request accepts an optional `secret_context` field. This field allows the client initiating the execution to provide sensitive data (like API keys, user tokens, etc.) that should be made available *only* to remote functions called by the script, but *not* directly accessible within the script's execution environment itself.

- When the runtime makes a remote function call (via `is_function_exists` followed by the function call request), the `secret_context` provided in the original `execute` request is included in the `FunctionCallRequest` sent to the client handling the remote function.
- The script running within the Hexput runtime cannot access the `secret_context` directly.
- This provides a secure way to pass credentials or sensitive configuration needed by the host environment (client) to fulfill remote function calls initiated by the sandboxed script.

Example usage in the client handling the remote call:

```javascript
// In the client's message handler for function calls
handleMessage(data) {
  const message = JSON.parse(data);
  
  if (message.function_name && message.arguments) {
    const handler = this.callHandlers[message.function_name];
    if (handler) {
      // Access secret context if needed by the handler
      const secretContext = message.secret_context; 
      console.log("Secret context received:", secretContext); 
      
      // Execute handler, potentially using secretContext
      // ... handler(...message.arguments, secretContext) ...
    }
    // ... rest of the handler ...
  }
  // ... other message handling ...
}
```

## Remote Function Calling

One of the most powerful features of Hexput Runtime is remote function calling. This capability allows code executing in the runtime to call functions that are implemented on the client side, enabling sandboxed code to safely interact with the host environment.

### How Remote Function Calling Works

1. **Function Discovery**: When the runtime encounters a function call that isn't defined in the local context (as a callback), it sends a function existence check (`is_function_exists`) request to the client, including a unique ID.
2. **Client Confirmation**: The client checks if it has a handler registered for the requested function name. It responds with a message containing the original ID and a boolean `exists` field.
3. **Function Execution Request**: If the client confirms the function exists (`exists: true`), the runtime sends a function call request. This includes a *new* unique ID, the function name, and the evaluated arguments.
4. **Client Execution & Response**: The client executes the function with the provided arguments and sends back a response message containing the call ID and the `result` (or an `error` if something went wrong).
5. **Runtime Integration**: The runtime receives the response, matches it to the pending call using the ID, and integrates the result (or error) back into the running code execution.
6. **Timeout Protection**: Both the function existence check and the function call have configurable timeouts to prevent hanging executions. If a timeout occurs or the client indicates the function doesn't exist, the runtime throws a `FunctionNotFoundError`.

### Remote Function Protocol Summary

1. **Check if Function Exists**:
   - Runtime sends: `{"id": "check-uuid", "action": "is_function_exists", "function_name": "myFunction"}`
   - Client responds: `{"id": "check-uuid", "exists": true}` or `{"id": "check-uuid", "exists": false}`

2. **Call Function (only if `exists` was true)**:
   - Runtime sends: `{"id": "call-uuid", "function_name": "myFunction", "arguments": [arg1, arg2, ...]}`
   - Client responds: `{"id": "call-uuid", "result": functionResult}` or `{"id": "call-uuid", "result": null, "error": "Error message"}`

### Example Implementation

This example shows how to implement a client that handles remote function calls according to the protocol:

```javascript
// ... (HexputClient class definition remains the same) ...

  handleMessage(data) {
    const message = JSON.parse(data);
    
    // Handle function existence check from server
    if (message.action === "is_function_exists") {
      const functionName = message.function_name;
      const exists = typeof this.callHandlers[functionName] === "function";
      console.log(`Runtime checking existence of '${functionName}': ${exists}`);
      this.ws.send(JSON.stringify({
        id: message.id, // Use the ID from the server's request
        exists: exists
      }));
      return;
    }
    
    // Handle function call request from server
    if (message.function_name && message.arguments) {
      const functionName = message.function_name;
      const handler = this.callHandlers[functionName];
      console.log(`Runtime calling function '${functionName}' with args:`, message.arguments);
      if (handler) {
        try {
          // Handle both sync and async handlers
          Promise.resolve(handler(...message.arguments))
            .then(result => {
              this.ws.send(JSON.stringify({
                id: message.id, // Use the ID from the server's request
                result: result === undefined ? null : result // Ensure result is not undefined
              }));
            })
            .catch(error => {
               console.error(`Error executing remote function '${functionName}':`, error);
               this.ws.send(JSON.stringify({
                 id: message.id,
                 result: null,
                 error: error instanceof Error ? error.message : String(error)
               }));
            });
        } catch (error) { // Catch synchronous errors
          console.error(`Synchronous error executing remote function '${functionName}':`, error);
          this.ws.send(JSON.stringify({
            id: message.id,
            result: null,
            error: error instanceof Error ? error.message : String(error)
          }));
        }
      } else {
        // Should ideally not happen if existence check works, but handle defensively
        console.warn(`Received call for unknown function '${functionName}'`);
         this.ws.send(JSON.stringify({
           id: message.id,
           result: null,
           error: `Function '${functionName}' not found on client.`
         }));
      }
      return;
    }
    
    // Handle response to our own requests (e.g., execute)
    if (message.id && this.responseHandlers[message.id]) {
      console.log(`Received response for request ID '${message.id}'`);
      this.responseHandlers[message.id](message);
      delete this.responseHandlers[message.id];
      return;
    }

    // Handle connection status messages or other types
    if (message.type === 'connection' && message.status === 'connected') {
        console.log("Successfully connected to Hexput Runtime.");
        return;
    }

    console.warn("Received unhandled message:", message);
  }

  // ... (registerFunction, execute methods remain the same) ...
}

// ... (Usage example remains the same) ...
```

### Handling Asynchronous Functions

The example client implementation already supports asynchronous functions (returning Promises) in handlers. The client will wait for the Promise to resolve or reject before sending the result back to the runtime.

```javascript
client.registerFunction("fetchUserData", async (userId) => {
  console.log(`Fetching user data for ${userId}`);
  // The client waits for this Promise to resolve
  const response = await fetch(`https://jsonplaceholder.typicode.com/users/${userId}`);
  if (!response.ok) {
      throw new Error(`Failed to fetch user data: ${response.statusText}`);
  }
  const data = await response.json();
  console.log(`User data fetched for ${userId}:`, data);
  return data; // This data will be sent back to the runtime
});

// In the runtime code:
// vl userData = fetchUserData(1); // Calls the async client function
// return userData.name;
```

### Security Considerations

When implementing remote function calling:

1. **Validate all inputs** coming from the runtime arguments.
2. **Limit function capabilities** to only what's necessary. Do not expose functions that could modify sensitive system state or files without careful checks.
3. **Handle timeouts** gracefully on the client side for potentially long-running operations, although the runtime also has its own timeout.
4. **Implement permission systems** if different levels of access are needed for functions called by the runtime.
5. **Avoid exposing sensitive internal functions** or data structures directly. Create specific, safe wrappers if needed.
6. **Log remote function calls** and their outcomes for monitoring and debugging.

By carefully implementing these patterns, you can safely bridge between sandboxed code and your application's functionality.

## Security Options

Hexput Runtime offers configurable security constraints via the `options` field in `parse` and `execute` requests to restrict what code can do:

- `no_object_constructions`: Prevents creating new objects (`{}`).
- `no_array_constructions`: Prevents creating new arrays (`[]`).
- `no_object_navigation`: Prevents accessing object properties (`obj.prop`, `obj['prop']`).
- `no_variable_declaration`: Prevents declaring new variables (`vl x = ...`).
- `no_loops`: Prevents using loop constructs (`loop item in list { ... }`).
- `no_object_keys`: Prevents getting object keys (`keysOf obj`).
- `no_callbacks`: Prevents defining (`callback name() { ... }`) and using callbacks.
- `no_conditionals`: Prevents using if/else statements (`if condition { ... }`).
- `no_return_statements`: Prevents using return statements (`return value`).
- `no_loop_control`: Prevents using break/continue (`end`, `continue`).
- `no_operators`: Prevents using mathematical operators (`+`, `-`, `*`, `/`).
- `no_equality`: Prevents using equality and comparison operators (`==`, `<`, `>`, `<=`, `>=`).
- `no_assignments`: Prevents assigning values to variables (`x = value`, `obj.prop = value`).

## Examples

### Basic Execution

Client code to execute a simple expression:

```javascript
const ws = new WebSocket('ws://localhost:9001');

ws.onopen = () => {
  console.log("WebSocket connected");
  ws.send(JSON.stringify({
    id: "req-1",
    action: "execute",
    code: "vl result = 5 + 10; return result;",
    options: {} // Default options (all features enabled)
  }));
};

ws.onmessage = (event) => {
  const response = JSON.parse(event.data);
  // Ignore connection message
  if (response.type === 'connection') return; 
  
  console.log('Execution result:', response);
  // Example output: { id: 'req-1', success: true, result: 15, error: null }
  ws.close(); 
};

ws.onerror = (error) => {
  console.error("WebSocket error:", error);
};

ws.onclose = () => {
  console.log("WebSocket closed");
};
```

### Function Bridging Example (using HexputClient class from above)

```javascript
// Assumes HexputClient class is defined as shown previously

const client = new HexputClient("ws://localhost:9001");

// Register a function the runtime can call
client.registerFunction("calculateTotal", (base, tax) => {
  console.log(`Client executing calculateTotal(${base}, ${tax})`);
  if (typeof base !== 'number' || typeof tax !== 'number') {
      throw new Error("Invalid arguments for calculateTotal");
  }
  return base + (base * tax);
});

// Wait for connection before executing
setTimeout(() => {
  if (client.ws.readyState === WebSocket.OPEN) {
    client.execute(`
      vl price = 100; 
      vl taxRate = 0.07;
      // This will trigger the remote function call protocol
      vl total = calculateTotal(price, taxRate); 
      return total;
    `)
    .then(result => {
      console.log("Execution result from runtime:", result); // Should be 107
    })
    .catch(error => {
      console.error("Execution error from runtime:", error);
    });
  } else {
    console.error("WebSocket not open. Cannot execute code.");
  }
}, 1000); // Simple delay to allow connection
```

## Built-in Methods

The runtime includes built-in methods for common data types, callable using member call syntax (e.g., `"hello".toUpperCase()`).

### String Methods
- `length()`, `len()`: Returns string length (number).
- `isEmpty()`: Checks if the string is empty (boolean).
- `substring(start, end)`: Extracts a portion of the string (string). `end` is optional. Indices are 0-based.
- `toLowerCase()`: Converts to lowercase (string).
- `toUpperCase()`: Converts to uppercase (string).
- `trim()`: Removes whitespace from both ends (string).
- `includes(substring)`, `contains(substring)`: Checks if string contains a substring (boolean).
- `startsWith(prefix)`: Checks if string starts with prefix (boolean).
- `endsWith(suffix)`: Checks if string ends with suffix (boolean).
- `indexOf(substring)`: Returns the position (0-based index) of the first occurrence, or -1 if not found (number).
- `split(delimiter)`: Splits string into an array of strings based on the delimiter (array).
- `replace(old, new)`: Replaces occurrences of `old` string with `new` string (string).

### Array Methods
- `length()`, `len()`: Returns array length (number).
- `isEmpty()`: Checks if the array is empty (boolean).
- `join(separator)`: Joins array elements into a string using the separator (string). Elements are converted to strings.
- `first()`: Returns the first element, or `null` if empty.
- `last()`: Returns the last element, or `null` if empty.
- `includes(item)`, `contains(item)`: Checks if array contains an item (uses simple equality check) (boolean).
- `slice(start, end)`: Extracts a portion of the array (array). `end` is optional. Indices are 0-based.

### Object Methods
- `keys()`: Returns an array of the object's property names (strings) (array).
- `values()`: Returns an array of the object's property values (array).
- `isEmpty()`: Checks if the object has no properties (boolean).
- `has(key)`: Checks if the object has a specific property key (string) (boolean).
- `entries()`: Returns an array of `[key, value]` pairs (array of arrays).

### Number Methods
- `toString()`: Converts the number to its string representation (string).
- `toFixed(digits)`: Formats the number using fixed-point notation (string). Requires one number argument for digits.
- `isInteger()`: Checks if the number is an integer (boolean).
- `abs()`: Returns the absolute value of the number (number).

### Boolean Methods
- `toString()`: Converts the boolean to `"true"` or `"false"` (string).

### Null Methods
- `toString()`: Returns the string `"null"` (string).

## License

[MIT License](LICENSE)
