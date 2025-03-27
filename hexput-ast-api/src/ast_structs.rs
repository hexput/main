use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SourceLocation {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl SourceLocation {
    pub fn new(start_line: usize, start_column: usize, end_line: usize, end_column: usize) -> Self {
        Self {
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }

    pub fn from_spans(source_code: &str, start_offset: usize, end_offset: usize) -> Self {
        let (start_line, start_column) = get_line_column(source_code, start_offset);
        let (end_line, end_column) = get_line_column(source_code, end_offset);
        Self::new(start_line, start_column, end_line, end_column)
    }
}


fn get_line_column(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    
    for (i, ch) in source.chars().enumerate() {
        if i >= offset {
            break;
        }
        
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    
    (line, column)
}


#[derive(Debug, Clone, Copy, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Program {
    #[serde(rename = "type")]
    pub node_type: String,
    pub statements: Vec<Statement>,
    pub location: SourceLocation,
}

impl Program {
    pub fn new(statements: Vec<Statement>, location: SourceLocation) -> Self {
        Self {
            node_type: "PROGRAM".to_string(),
            statements,
            location,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum Statement {
    #[serde(rename = "VARIABLE_DECLARATION")]
    VariableDeclaration {
        name: String,
        value: Expression,
        location: SourceLocation,
    },
    #[serde(rename = "EXPRESSION_STATEMENT")]
    ExpressionStatement {
        expression: Expression,
        location: SourceLocation,
    },
    #[serde(rename = "IF_STATEMENT")]
    IfStatement {
        condition: Expression,
        body: Block,
        #[serde(skip_serializing_if = "Option::is_none")]
        else_body: Option<Block>,
        location: SourceLocation,
    },
    #[serde(rename = "BLOCK")]
    Block { 
        block: Block,
        location: SourceLocation,
    },
    #[serde(rename = "CALLBACK_DECLARATION")]
    CallbackDeclaration {
        name: String,
        params: Vec<String>,
        body: Block,
        location: SourceLocation,
    },
    #[serde(rename = "RETURN_STATEMENT")]
    ReturnStatement {
        value: Expression,
        location: SourceLocation,
    },
    #[serde(rename = "LOOP_STATEMENT")]
    LoopStatement {
        variable: String,
        iterable: Expression,
        body: Block,
        location: SourceLocation,
    },
    #[serde(rename = "END_STATEMENT")]
    EndStatement {
        location: SourceLocation,
    },
    #[serde(rename = "CONTINUE_STATEMENT")]
    ContinueStatement {
        location: SourceLocation,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Block {
    #[serde(rename = "type")]
    pub node_type: String,
    pub statements: Vec<Statement>,
    pub location: SourceLocation,
}

impl Block {
    pub fn new(statements: Vec<Statement>, location: SourceLocation) -> Self {
        Self {
            node_type: "BLOCK".to_string(),
            statements,
            location,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum Expression {
    #[serde(rename = "STRING_LITERAL")]
    StringLiteral {
        value: String,
        location: SourceLocation,
    },
    #[serde(rename = "NUMBER_LITERAL")]
    NumberLiteral {
        value: f64,
        location: SourceLocation,
    },
    #[serde(rename = "IDENTIFIER")]
    Identifier {
        name: String,
        location: SourceLocation,
    },
    #[serde(rename = "BINARY_EXPRESSION")]
    BinaryExpression {
        left: Box<Expression>,
        operator: Operator,
        right: Box<Expression>,
        location: SourceLocation,
    },
    #[serde(rename = "ASSIGNMENT_EXPRESSION")]
    AssignmentExpression {
        target: String,
        value: Box<Expression>,
        location: SourceLocation,
    },
    #[serde(rename = "MEMBER_ASSIGNMENT_EXPRESSION")]
    MemberAssignmentExpression {
        object: Box<Expression>,
        #[serde(skip_serializing_if = "Option::is_none")]
        property: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        property_expr: Option<Box<Expression>>,
        computed: bool,  
        value: Box<Expression>,
        location: SourceLocation,
    },
    #[serde(rename = "CALL_EXPRESSION")]
    CallExpression {
        callee: String,
        arguments: Vec<Expression>,
        location: SourceLocation,
    },
    #[serde(rename = "MEMBER_CALL_EXPRESSION")]
    MemberCallExpression {
        object: Box<Expression>,
        #[serde(skip_serializing_if = "Option::is_none")]
        property: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        property_expr: Option<Box<Expression>>,
        computed: bool,
        arguments: Vec<Expression>,
        location: SourceLocation,
    },
    #[serde(rename = "INLINE_CALLBACK_EXPRESSION")]
    InlineCallbackExpression {
        name: String,
        params: Vec<String>,
        body: Block,
        location: SourceLocation,
    },
    #[serde(rename = "ARRAY_EXPRESSION")]
    ArrayExpression {
        elements: Vec<Expression>,
        location: SourceLocation,
    },
    #[serde(rename = "OBJECT_EXPRESSION")]
    ObjectExpression {
        properties: Vec<Property>,
        location: SourceLocation,
    },
    #[serde(rename = "MEMBER_EXPRESSION")]
    MemberExpression {
        object: Box<Expression>,
        #[serde(skip_serializing_if = "Option::is_none")]
        property: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        property_expr: Option<Box<Expression>>,
        computed: bool,  
        location: SourceLocation,
    },
    #[serde(rename = "KEYS_OF_EXPRESSION")]
    KeysOfExpression {
        object: Box<Expression>,
        location: SourceLocation,
    },
    #[serde(rename = "BOOLEAN_LITERAL")]
    BooleanLiteral {
        value: bool,
        location: SourceLocation,
    },
    #[serde(rename = "UNARY_EXPRESSION")]
    UnaryExpression {
        operator: UnaryOperator,
        operand: Box<Expression>,
        location: SourceLocation,
    },
    #[serde(rename = "NULL_LITERAL")]
    NullLiteral {
        location: SourceLocation,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Property {
    #[serde(rename = "type")]
    pub node_type: String,
    pub key: String,
    pub value: Expression,
    pub location: SourceLocation,
}

impl Property {
    pub fn new(key: String, value: Expression, location: SourceLocation) -> Self {
        Self {
            node_type: "PROPERTY".to_string(),
            key,
            value,
            location,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Operator {
    Equal,
    NotEqual,
    Plus,
    Minus,
    Multiply,
    Divide,
    Greater,
    Less,
    GreaterEqual,
    LessEqual,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum UnaryOperator {
    Not,
}

