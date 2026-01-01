//! Execution limits

#[derive(Debug, Clone)]
pub struct Limits {
    /// Maximum number of instructions to execute
    pub max_instructions: usize,
    
    /// Maximum recursion depth
    pub max_recursion_depth: usize,
    
    /// Maximum execution time in milliseconds
    pub max_execution_time_ms: u64,
    
    /// Maximum memory allocation (future)
    pub max_memory_bytes: usize,
}

impl Limits {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_instructions(mut self, max: usize) -> Self {
        self.max_instructions = max;
        self
    }

    pub fn with_max_recursion_depth(mut self, max: usize) -> Self {
        self.max_recursion_depth = max;
        self
    }

    pub fn with_max_execution_time(mut self, ms: u64) -> Self {
        self.max_execution_time_ms = ms;
        self
    }

    /// Unlimited execution (dangerous, for testing only)
    pub fn unlimited() -> Self {
        Self {
            max_instructions: usize::MAX,
            max_recursion_depth: usize::MAX,
            max_execution_time_ms: u64::MAX,
            max_memory_bytes: usize::MAX,
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_instructions: 100_000,
            max_recursion_depth: 100,
            max_execution_time_ms: 5000,
            max_memory_bytes: 10 * 1024 * 1024, // 10MB
        }
    }
}
