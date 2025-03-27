```ts
cargo run -p hexput-ast-cli -r -- --no-source-mapping :: '
vl config = {
  name: "DataProcessor",
  version: 1.5,
  enabled: true,
  settings: { parallel: true, maxThreads: 8, timeout: 5000 },
  supportedFormats: ["json", "csv", "xml"]
};

cb getConfigValue(key, defaultValue) {
  vl value = config[key];
  
  if value == "undefined" {
    res defaultValue;
  }
  
  res value;
}

res getConfigValue("name", "defaultName");
'
```

```ts
cargo run -p hexput-ast-cli -r -- --no-callbacks :: '
vl config = {
  name: "DataProcessor",
  version: 1.5,
  enabled: true,
  settings: { parallel: true, maxThreads: 8, timeout: 5000 },
  supportedFormats: ["json", "csv", "xml"]
};

cb getConfigValue(key, defaultValue) {
  vl value = config[key];
  
  if value == "undefined" {
    res defaultValue;
  }
  
  res value;
}

res getConfigValue("name", "defaultName");
'
```

```ts
cargo run -p hexput-ast-cli -r -- --no-source-mapping :: '
cb transformData(item, options) {
  vl result = {};
  
  if options.uppercase {
    result.value = item + " - PROCESSED";
  } else {
    result.value = item + " - processed";
    result.original = item;
  }
  
  result.timestamp = 1234567890;
  
  res result;
}

vl options = { uppercase: true };
res transformData("sample", options);
'
```

```ts
cargo run -p hexput-ast-cli -r -- --no-source-mapping :: '
// Define a complex object with nested properties
vl config = {
  name: "DataProcessor",
  version: 1.5,
  enabled: true,
  settings: {
    parallel: true,
    maxThreads: 8,
    timeout: 5000
  },
  supportedFormats: ["json", "csv", "xml"]
};

// Define a complex transformer callback
cb transformData(item, options) {
  vl result = {};
  
  if options.uppercase {
    result.value = item + " - PROCESSED";
  } else {
    result.value = item + " - processed";
    result.original = item;
  }
  
  result.timestamp = 1234567890;
  
  res result;
}

// Define a helper callback
cb getConfigValue(key, defaultValue) {
  vl value = config[key];
  
  if value == "undefined" {
    res defaultValue;
  }
  
  res value;
}

// Get configuration and prepare data
vl formats = config.supportedFormats;
vl options = {
  uppercase: config.settings.parallel == true,
  includeMetadata: true
};

// Process each format using our callback
vl results = [];
vl index = 0;

loop format in formats {
  // Skip XML format
  if format == "xml" {
    continue;
  }
  
  // Process the current format
  vl processedItem = transformData(format, options);
  
  // Store in results array
  results[index] = processedItem;
  
  // Calculate score based on format length
  vl score = 0;
  vl nameLength = 0;
  
  // Simulate complex calculation
  loop char in format {
    nameLength = nameLength + 1;
    score = score + nameLength * 2;
  }
  
  // Add score to results
  results[index].score = score;
  
  // Add format-specific metadata
  if format == "json" {
    results[index].metadata = {
      parser: "fastjson",
      popularity: 10 * 4 / 2
    };
  }
  
  // Move to next index
  index = index + 1;
  
  // End early if we find CSV
  if format == "csv" {
    end;
  }
}

// Get all keys from first result for analytics
vl firstResultKeys = keysof results[0];

// Build a summary of our processing
vl summary = {
  processed: index,
  timestamp: 987654321,
  configuration: config.settings,
  keys: firstResultKeys,
  success: true
};

// Add everything to final output object
vl output = {
  status: "complete",
  results: results,
  summary: summary,
  config: config
};

// Output is implicitly returned
res output;
'
```