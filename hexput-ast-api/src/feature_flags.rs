use serde::Serialize;


#[derive(Debug, Clone, Copy, Serialize)]
pub struct FeatureFlags {
    pub allow_variable_declaration: bool,
    pub allow_conditionals: bool,
    pub allow_loops: bool,
    pub allow_callbacks: bool,
    pub allow_return_statements: bool,
    pub allow_loop_control: bool,
    pub allow_assignments: bool,
    pub allow_object_navigation: bool,
    pub allow_array_constructions: bool,
    pub allow_object_constructions: bool,
    pub allow_object_keys: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        
        Self {
            allow_variable_declaration: true,
            allow_conditionals: true,
            allow_loops: true,
            allow_callbacks: true,
            allow_return_statements: true,
            allow_loop_control: true,
            allow_assignments: true,          
            allow_object_navigation: true,    
            allow_array_constructions: true,
            allow_object_constructions: true,
            allow_object_keys: true,
        }
    }
}

impl FeatureFlags {
    pub fn all_enabled() -> Self {
        Self::default()
    }
    
    
    pub fn all_disabled() -> Self {
        Self {
            allow_variable_declaration: false,
            allow_conditionals: false,
            allow_loops: false,
            allow_callbacks: false,
            allow_return_statements: false,
            allow_loop_control: false,
            allow_assignments: false,
            allow_object_navigation: false,
            allow_array_constructions: false,
            allow_object_constructions: false,
            allow_object_keys: false,
        }
    }
    
    
    pub fn expressions_only() -> Self {
        let mut flags = Self::all_disabled();
        flags.allow_assignments = true;
        flags.allow_object_navigation = true;
        flags
    }

}
