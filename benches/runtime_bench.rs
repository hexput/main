use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use hexput::{parse, runtime::Context, sandbox::Limits};
use std::hint::black_box;

fn bench_parse(c: &mut Criterion) {
    let source = r#"
        vl x = 42;
        vl y = 100;
        vl result = x + y;
        res result;
    "#;

    c.bench_function("parse_simple", |b| b.iter(|| parse(black_box(source))));
}

fn bench_non_cached_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("non_cached_execution");

    // Simple arithmetic - parse + execute on every iteration
    let simple_source = r#"
        vl x = 42;
        vl y = 100;
        vl result = x + y;
        res result;
    "#;

    group.bench_function("simple_arithmetic", |b| {
        b.iter(|| {
            let ast = parse(simple_source).unwrap();
            let limits = Limits::unlimited();
            let mut context = Context::new(limits);
            hexput::runtime::vm::execute(&ast, &mut context)
        })
    });

    // Loop execution - parse + execute on every iteration
    let loop_source = r#"
        vl sum = 0;
        vl numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        loop n in numbers {
            sum = sum + n;
        }
        res sum;
    "#;

    group.bench_function("loop_sum", |b| {
        b.iter(|| {
            let ast = parse(loop_source).unwrap();
            let limits = Limits::unlimited();
            let mut context = Context::new(limits);
            hexput::runtime::vm::execute(&ast, &mut context)
        })
    });

    // Complex nested operations - parse + execute on every iteration
    let complex_source = r#"
        vl data = {
            values: [10, 20, 30, 40, 50],
            multiplier: 2
        };
        
        vl result = [];
        vl index = 0;
        
        loop val in data.values {
            vl processed = val * data.multiplier;
            result[index] = processed;
            index = index + 1;
        }
        
        res result;
    "#;

    group.bench_function("complex_nested", |b| {
        b.iter(|| {
            let ast = parse(complex_source).unwrap();
            let limits = Limits::unlimited();
            let mut context = Context::new(limits);
            hexput::runtime::vm::execute(&ast, &mut context)
        })
    });

    // Callback execution - parse + execute on every iteration
    let callback_source = r#"
        cb double(x) {
            res x * 2;
        }
        
        vl numbers = [1, 2, 3, 4, 5];
        vl results = [];
        vl index = 0;
        
        loop n in numbers {
            results[index] = double(n);
            index = index + 1;
        }
        
        res results;
    "#;

    group.bench_function("with_callbacks", |b| {
        b.iter(|| {
            let ast = parse(callback_source).unwrap();
            let limits = Limits::unlimited();
            let mut context = Context::new(limits);
            hexput::runtime::vm::execute(&ast, &mut context)
        })
    });

    group.finish();
}

fn bench_cached_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("cached_execution");

    // Simple arithmetic with cached AST - only execute
    let simple_source = r#"
        vl x = 42;
        vl y = 100;
        vl result = x + y;
        res result;
    "#;
    let simple_ast = parse(simple_source).unwrap();

    group.bench_function("simple_arithmetic", |b| {
        b.iter(|| {
            let limits = Limits::unlimited();
            let mut context = Context::new(limits);
            hexput::runtime::vm::execute(&simple_ast, &mut context)
        })
    });

    // Loop execution with cached AST - only execute
    let loop_source = r#"
        vl sum = 0;
        vl numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        loop n in numbers {
            sum = sum + n;
        }
        res sum;
    "#;
    let loop_ast = parse(loop_source).unwrap();

    group.bench_function("loop_sum", |b| {
        b.iter(|| {
            let limits = Limits::unlimited();
            let mut context = Context::new(limits);
            hexput::runtime::vm::execute(&loop_ast, &mut context)
        })
    });

    // Complex nested operations with cached AST - only execute
    let complex_source = r#"
        vl data = {
            values: [10, 20, 30, 40, 50],
            multiplier: 2
        };
        
        vl result = [];
        vl index = 0;
        
        loop val in data.values {
            vl processed = val * data.multiplier;
            result[index] = processed;
            index = index + 1;
        }
        
        res result;
    "#;
    let complex_ast = parse(complex_source).unwrap();

    group.bench_function("complex_nested", |b| {
        b.iter(|| {
            let limits = Limits::unlimited();
            let mut context = Context::new(limits);
            hexput::runtime::vm::execute(&complex_ast, &mut context)
        })
    });

    // Callback execution with cached AST - only execute
    let callback_source = r#"
        cb double(x) {
            res x * 2;
        }
        
        vl numbers = [1, 2, 3, 4, 5];
        vl results = [];
        vl index = 0;
        
        loop n in numbers {
            results[index] = double(n);
            index = index + 1;
        }
        
        res results;
    "#;
    let callback_ast = parse(callback_source).unwrap();

    group.bench_function("with_callbacks", |b| {
        b.iter(|| {
            let limits = Limits::unlimited();
            let mut context = Context::new(limits);
            hexput::runtime::vm::execute(&callback_ast, &mut context)
        })
    });

    group.finish();
}

fn bench_parse_vs_execute(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_vs_execute_breakdown");

    let source = r#"
        vl sum = 0;
        vl numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        loop n in numbers {
            sum = sum + n;
        }
        res sum;
    "#;

    // Parse only
    group.bench_function("parse_only", |b| b.iter(|| parse(black_box(source))));

    // Execute only (pre-parsed)
    let ast = parse(source).unwrap();
    group.bench_function("execute_only", |b| {
        b.iter(|| {
            let limits = Limits::unlimited();
            let mut context = Context::new(limits);
            hexput::runtime::vm::execute(&ast, &mut context)
        })
    });

    // Parse + Execute (non-cached)
    group.bench_function("parse_and_execute", |b| {
        b.iter(|| {
            let ast = parse(source).unwrap();
            let limits = Limits::unlimited();
            let mut context = Context::new(limits);
            hexput::runtime::vm::execute(&ast, &mut context)
        })
    });

    group.finish();
}

fn bench_execution_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_scaling");

    // Benchmark how cached execution scales with different script complexities
    for size in [10, 50, 100].iter() {
        // Generate an array of numbers
        let numbers: Vec<i32> = (0..*size).collect();
        let numbers_str = numbers
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let source = format!(
            r#"
            vl sum = 0;
            vl numbers = [{}];
            loop n in numbers {{
                sum = sum + n;
            }}
            res sum;
            "#,
            numbers_str
        );

        let ast = parse(&source).unwrap();

        group.bench_with_input(BenchmarkId::new("cached", size), size, |b, _| {
            b.iter(|| {
                let limits = Limits::unlimited();
                let mut context = Context::new(limits);
                hexput::runtime::vm::execute(&ast, &mut context)
            })
        });

        group.bench_with_input(BenchmarkId::new("non_cached", size), size, |b, _| {
            b.iter(|| {
                let ast = parse(&source).unwrap();
                let limits = Limits::unlimited();
                let mut context = Context::new(limits);
                hexput::runtime::vm::execute(&ast, &mut context)
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_non_cached_execution,
    bench_cached_execution,
    bench_parse_vs_execute,
    bench_execution_scaling
);
criterion_main!(benches);
