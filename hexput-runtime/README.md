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
cd hexput-main

# Build the project
cargo build --r -p hexput-runtime

# Run the server
./target/release/hexput-runtime
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

#### Requests

The server accepts the following request types:

1. **Parse Request**:
```json
{
  "id": "unique-request-id",
  "action": "parse",
  "code": "let x = 10;",
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
  "code": "let x = 10; return x * 2;",
  "options": {
    "no_loops": true,
    "no_callbacks": true
  },
  "context": {
    "initialValue": 5
  }
}
```

3. **Function Check Request**:
```json
{
  "id": "unique-request-id",
  "action": "is_function_exists",
  "function_name": "calculateTotal"
}
```

4. **Function Call Request**:
```json
{
  "id": "unique-request-id",
  "function_name": "calculateTotal",
  "arguments": [10, 20, {"tax": 0.05}]
}
```

#### Responses

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

4. **Function Exists Response**:
```json
{
  "id": "unique-request-id",
  "exists": true
}
```

5. **Function Call Response**:
```json
{
  "id": "unique-request-id",
  "result": { /* Function result */ }
}
```

## Remote Function Calling

One of the most powerful features of Hexput Runtime is remote function calling. This capability allows code executing in the runtime to call functions that are implemented on the client side, enabling sandboxed code to safely interact with the host environment.

### How Remote Function Calling Works

1. **Function Discovery**: When the runtime encounters a function call that isn't defined in the local context, it sends a function existence check to the client.
2. **Function Execution**: If the function exists on the client side, the runtime sends a function call request with arguments.
3. **Response Handling**: The client executes the function and returns the result, which the runtime then integrates into the running code.
4. **Timeout Protection**: Function calls have configurable timeouts to prevent hanging executions.

### Remote Function Protocol

1. **Check if Function Exists**:
   - Runtime sends: `{"id": "uuid", "action": "is_function_exists", "function_name": "myFunction"}`
   - Client responds: `{"id": "uuid", "exists": true}`

2. **Call Function**:
   - Runtime sends: `{"id": "uuid", "function_name": "myFunction", "arguments": [arg1, arg2, ...]}`
   - Client responds: `{"id": "uuid", "result": functionResult}`

### Example Implementation

This example shows how to implement a client that handles remote function calls:

```javascript
class HexputClient {
  constructor(url) {
    this.url = url;
    this.callHandlers = {};
    this.responseHandlers = {};
    this.connect();
  }

  connect() {
    this.ws = new WebSocket(this.url);
    this.ws.onopen = () => console.log("Connected to Hexput Runtime");
    this.ws.onmessage = (event) => this.handleMessage(event.data);
    this.ws.onerror = (error) => console.error("WebSocket error:", error);
    this.ws.onclose = () => {
      console.log("Connection closed, reconnecting in 3s...");
      setTimeout(() => this.connect(), 3000);
    };
  }

  handleMessage(data) {
    const message = JSON.parse(data);
    
    // Handle function existence check
    if (message.action === "is_function_exists") {
      const exists = typeof this.callHandlers[message.function_name] === "function";
      this.ws.send(JSON.stringify({
        id: message.id,
        exists: exists
      }));
      return;
    }
    
    // Handle function call
    if (message.function_name && message.arguments) {
      const handler = this.callHandlers[message.function_name];
      if (handler) {
        try {
          const result = handler(...message.arguments);
          this.ws.send(JSON.stringify({
            id: message.id,
            result: result
          }));
        } catch (error) {
          this.ws.send(JSON.stringify({
            id: message.id,
            result: null,
            error: error.message
          }));
        }
      }
      return;
    }
    
    // Handle response to our requests
    if (message.id && this.responseHandlers[message.id]) {
      this.responseHandlers[message.id](message);
      delete this.responseHandlers[message.id];
      return;
    }
  }

  // Register a function that can be called by the runtime
  registerFunction(name, handler) {
    this.callHandlers[name] = handler;
  }

  // Execute code in the runtime
  execute(code, options = {}, context = {}) {
    return new Promise((resolve, reject) => {
      const id = `exec-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
      
      this.responseHandlers[id] = (response) => {
        if (response.error) {
          reject(new Error(response.error));
        } else {
          resolve(response.result);
        }
      };
      
      this.ws.send(JSON.stringify({
        id: id,
        action: "execute",
        code: code,
        options: options,
        context: context
      }));
    });
  }
}

// Usage example
const client = new HexputClient("ws://localhost:9001");

// Register functions that can be called from the runtime
client.registerFunction("calculateTotal", (price, taxRate) => {
  console.log(`Calculating total for price ${price} and tax rate ${taxRate}`);
  return price + (price * taxRate);
});

client.registerFunction("getCurrentDate", () => {
  return new Date().toISOString();
});

// Execute code that calls the registered functions
client.execute(`
  let price = 100;
  let taxRate = 0.07;
  let total = calculateTotal(price, taxRate);
  let orderDate = getCurrentDate();
  
  return {
    price: price,
    taxRate: taxRate,
    total: total,
    date: orderDate
  };
`)
.then(result => {
  console.log("Execution result:", result);
})
.catch(error => {
  console.error("Execution error:", error);
});
```

### Handling Asynchronous Functions

For asynchronous operations, you can use Promises in your function handlers:

```javascript
client.registerFunction("fetchUserData", async (userId) => {
  // The client waits for this Promise to resolve
  const response = await fetch(`https://api.example.com/users/${userId}`);
  const data = await response.json();
  return data;
});

// In the runtime code
// let userData = fetchUserData("user123");
```

### Security Considerations

When implementing remote function calling:

1. **Validate all inputs** coming from the runtime
2. **Limit function capabilities** to only what's necessary
3. **Handle timeouts** for long-running operations
4. **Consider implementing permission systems** for different functions
5. **Avoid exposing sensitive functions** that could be misused

By carefully implementing these patterns, you can safely bridge between sandboxed code and your application's functionality.

## Security Options

Hexput Runtime offers configurable security constraints to restrict what code can do:

- `no_object_constructions`: Prevents creating new objects
- `no_array_constructions`: Prevents creating new arrays
- `no_object_navigation`: Prevents accessing object properties
- `no_variable_declaration`: Prevents declaring new variables
- `no_loops`: Prevents using loop constructs
- `no_object_keys`: Prevents getting object keys
- `no_callbacks`: Prevents defining and using callbacks
- `no_conditionals`: Prevents using if/else statements
- `no_return_statements`: Prevents using return statements
- `no_loop_control`: Prevents using break/continue
- `no_operators`: Prevents using mathematical operators
- `no_equality`: Prevents using equality operators
- `no_assignments`: Prevents assigning values to variables

## Examples

### Basic Execution

Client code to execute a simple expression:

```javascript
const ws = new WebSocket('ws://localhost:9001');

ws.onopen = () => {
  ws.send(JSON.stringify({
    id: "req-1",
    action: "execute",
    code: "let result = 5 + 10; return result;",
    options: {}
  }));
};

ws.onmessage = (event) => {
  const response = JSON.parse(event.data);
  console.log('Execution result:', response);
};
```

### Function Bridging

Server-side function execution from client code:

```javascript
// Client-side handler for server function calls
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  
  // Handle function call requests
  if (message.function_name === "calculateTotal") {
    const [base, tax] = message.arguments;
    const total = base + (base * tax);
    
    // Send the result back
    ws.send(JSON.stringify({
      id: message.id,
      result: total
    }));
  }
};

// Execute code that calls client-side functions
ws.send(JSON.stringify({
  id: "req-2",
  action: "execute",
  code: "let price = 100; let result = calculateTotal(price, 0.07); return result;",
  options: {}
}));
```

## Built-in Methods

The runtime includes built-in methods for common data types:

### String Methods
- `length()`, `len()`: Returns string length
- `isEmpty()`: Checks if the string is empty
- `substring(start, end)`: Extracts a portion of the string
- `toLowerCase()`: Converts to lowercase
- `toUpperCase()`: Converts to uppercase
- `trim()`: Removes whitespace from both ends
- `includes(substring)`, `contains(substring)`: Checks if string contains a substring
- `startsWith(prefix)`: Checks if string starts with prefix
- `endsWith(suffix)`: Checks if string ends with suffix
- `indexOf(substring)`: Returns the position of the first occurrence
- `split(delimiter)`: Splits string into an array

### Array Methods
- `length()`, `len()`: Returns array length
- `isEmpty()`: Checks if the array is empty
- `join(separator)`: Joins array elements into a string
- `first()`: Returns the first element
- `last()`: Returns the last element
- `includes(item)`, `contains(item)`: Checks if array contains an item
- `slice(start, end)`: Extracts a portion of the array

### Object Methods
- `keys()`: Returns an array of property names
- `values()`: Returns an array of property values
- `isEmpty()`: Checks if the object has no properties
- `has(key)`: Checks if the object has a specific property
- `entries()`: Returns an array of [key, value] pairs

## License

[MIT License](LICENSE)
