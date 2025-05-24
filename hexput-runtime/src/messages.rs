use hexput_ast_api::feature_flags::FeatureFlags;
use serde::{Deserialize, Serialize, de::Deserializer};

#[derive(Debug, Clone)]
pub enum WebSocketMessage {
    Request(WebSocketRequest),
    FunctionResponse(FunctionCallResponse),
    FunctionExistsResponse(FunctionExistsResponse),
    Unknown(serde_json::Value)
}

impl<'de> Deserialize<'de> for WebSocketMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        
        if let serde_json::Value::Object(ref map) = value {
            if map.contains_key("id") && !map.contains_key("action") {
                if map.contains_key("exists") {
                    if let Ok(response) = serde_json::from_value::<FunctionExistsResponse>(value.clone()) {
                        return Ok(WebSocketMessage::FunctionExistsResponse(response));
                    }
                }
                
                if let Ok(response) = serde_json::from_value::<FunctionCallResponse>(value.clone()) {
                    return Ok(WebSocketMessage::FunctionResponse(response));
                }
            }
            
            if map.contains_key("action") {
                if let Ok(request) = serde_json::from_value::<WebSocketRequest>(value.clone()) {
                    return Ok(WebSocketMessage::Request(request));
                }
            }
        }
        
        Ok(WebSocketMessage::Unknown(value))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSocketRequest {
    pub id: String,
    pub action: String,
    pub code: String,
    #[serde(default)]
    pub options: AstParserOptions,
    #[serde(default)]
    pub context: serde_json::Map<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_context: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSocketResponse {
    pub id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorLocation {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl From<hexput_ast_api::ast_structs::SourceLocation> for ErrorLocation {
    fn from(loc: hexput_ast_api::ast_structs::SourceLocation) -> Self {
        ErrorLocation {
            line: loc.start_line,
            column: loc.start_column,
            end_line: loc.end_line,
            end_column: loc.end_column,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FunctionCallRequest {
    pub id: String,
    pub function_name: String,
    pub arguments: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_context: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FunctionCallResponse {
    pub id: String,
    pub result: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FunctionExistsRequest {
    pub id: String,
    pub action: String,
    pub function_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FunctionExistsResponse {
    pub id: String,
    pub exists: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecutionResult {
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct AstParserOptions {
    #[serde(default = "default_true")]
    pub minify: bool,
    #[serde(default)]
    pub include_source_mapping: bool,
    #[serde(default)]
    pub no_object_constructions: bool,
    #[serde(default)]
    pub no_array_constructions: bool,
    #[serde(default)]
    pub no_object_navigation: bool,
    #[serde(default)]
    pub no_variable_declaration: bool,
    #[serde(default)]
    pub no_loops: bool,
    #[serde(default)]
    pub no_object_keys: bool,
    #[serde(default)]
    pub no_callbacks: bool,
    #[serde(default)]
    pub no_conditionals: bool,
    #[serde(default)]
    pub no_return_statements: bool,
    #[serde(default)]
    pub no_loop_control: bool,
    #[serde(default)]
    pub no_operators: bool,
    #[serde(default)]
    pub no_equality: bool,
    #[serde(default)]
    pub no_assignments: bool,
}

fn default_true() -> bool {
    true
}

impl AstParserOptions {
    pub fn to_feature_flags(&self) -> FeatureFlags {
        FeatureFlags {
            allow_object_constructions: !self.no_object_constructions,
            allow_array_constructions: !self.no_array_constructions,
            allow_object_navigation: !self.no_object_navigation,
            allow_variable_declaration: !self.no_variable_declaration,
            allow_loops: !self.no_loops,
            allow_object_keys: !self.no_object_keys,
            allow_callbacks: !self.no_callbacks,
            allow_conditionals: !self.no_conditionals,
            allow_return_statements: !self.no_return_statements,
            allow_loop_control: !self.no_loop_control,
            allow_assignments: !self.no_assignments,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CallbackValue {
    pub callback_type: String, // "callback_reference"
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct CallbackFunction {
    pub name: String,
    pub params: Vec<String>,
    pub body: hexput_ast_api::ast_structs::Block,
}
