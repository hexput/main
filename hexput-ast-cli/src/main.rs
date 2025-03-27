use hexput_ast_api::feature_flags::FeatureFlags;
use clap::{Arg, Command, ArgAction};
use std::env;
use std::process;

fn main() {
    let matches = Command::new("ast-resolver-cli")
        .version("0.1.0")
        .about("AST resolver for a custom scripting language")
        .arg(Arg::new("code")
            .help("Code to parse")
            .action(ArgAction::Set))
        .arg(Arg::new("minify")
            .long("minify")
            .help("Minify the output JSON (remove whitespace)")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("no-object-constructions")
            .long("no-object-constructions")
            .help("Disable object literal construction")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("no-array-constructions")
            .long("no-array-constructions")
            .help("Disable array literal construction")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("no-object-navigation")
            .long("no-object-navigation")
            .help("Disable object property access (dot notation and bracket notation)")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-variable-declaration")
            .long("no-variable-declaration")
            .help("Disable variable declarations with 'vl'")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-loops")
            .long("no-loops")
            .help("Disable loop statements")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-object-keys")
            .long("no-object-keys")
            .help("Disable keysof operator")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-callbacks")
            .long("no-callbacks")
            .help("Disable callback declarations")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-conditionals")
            .long("no-conditionals")
            .help("Disable if statements")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-return-statements")
            .long("no-return-statements")
            .help("Disable return statements with 'res'")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-loop-control")
            .long("no-loop-control")
            .help("Disable loop control statements (end, continue)")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-operators")
            .long("no-operators")
            .help("Disable arithmetic operators (+, *, /)")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-equality")
            .long("no-equality")
            .help("Disable equality operator (==)")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-assignments")
            .long("no-assignments")
            .help("Disable assignment operator (=)")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("no-source-mapping")
            .long("no-source-mapping")
            .help("Disable source location information in the output JSON")
            .action(ArgAction::SetTrue))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .allow_external_subcommands(true)
        .get_matches();

    let args: Vec<String> = env::args().collect();
    
    let code = extract_code_from_args(&args);
    
    let feature_flags = create_feature_flags_from_cli_args(&matches);
    
    let minify = matches.get_flag("minify");
    
    let include_source_mapping = !matches.get_flag("no-source-mapping");
    
    match hexput_ast_api::process_code(&code, feature_flags) {
        Ok(program) => {
            let json_result = if minify {
                hexput_ast_api::to_json_string(&program, include_source_mapping)
            } else {
                hexput_ast_api::to_json_string_pretty(&program, include_source_mapping)
            };
            
            match json_result {
                Ok(json) => {
                    println!("{}", json);
                },
                Err(e) => {
                    eprintln!("Error serializing AST to JSON: {}", e);
                    process::exit(1);
                }
            }
        }
        Err(e) => {
            let error_json = hexput_ast_api::format_error_as_json(&e, minify);
            eprintln!("{}", error_json);
            process::exit(1);
        }
    }
}

fn extract_code_from_args(args: &[String]) -> String {
    if let Some(pos) = args.iter().position(|arg| arg == "::") {
        if pos + 1 < args.len() {
            args[(pos + 1)..].join(" ")
        } else {
            eprintln!("No code provided after '::'");
            process::exit(1);
        }
    } else if args.len() > 1 {
        let first_non_flag = args.iter()
            .skip(1)
            .position(|arg| !arg.starts_with("--"))
            .map(|pos| pos + 1);
            
        if let Some(pos) = first_non_flag {
            args[pos].clone()
        } else {
            eprintln!("No code provided. Use --help for usage information.");
            process::exit(1);
        }
    } else {
        eprintln!("No code provided. Use --help for usage information.");
        process::exit(1);
    }
}

fn create_feature_flags_from_cli_args(args: &clap::ArgMatches) -> FeatureFlags {
    FeatureFlags {
        allow_object_constructions: !args.get_flag("no-object-constructions"),
        allow_array_constructions: !args.get_flag("no-array-constructions"),
        allow_object_navigation: !args.get_flag("no-object-navigation"),
        allow_variable_declaration: !args.get_flag("no-variable-declaration"),
        allow_loops: !args.get_flag("no-loops"),
        allow_object_keys: !args.get_flag("no-object-keys"),
        allow_callbacks: !args.get_flag("no-callbacks"),
        allow_conditionals: !args.get_flag("no-conditionals"),
        allow_return_statements: !args.get_flag("no-return-statements"),
        allow_loop_control: !args.get_flag("no-loop-control"),
        allow_assignments: !args.get_flag("no-assignments"),
    }
}