use criterion::*;

mod iteration;
mod iteration_term;

fn iter_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_simple");
    group.warm_up_time(std::time::Duration::from_millis(500));
    group.measurement_time(std::time::Duration::from_secs(4));

    group.bench_function("base", |b| {
        let mut bench = iteration::iter_simple::Benchmark::new();
        b.iter(move || bench.run());
    });
    group.bench_function("base_term", |b| {
        let mut bench = iteration_term::iter_simple::Benchmark::new();
        b.iter(move || bench.run());
    });

    group.bench_function("wide", |b| {
        let mut bench = iteration::iter_simple_wide::Benchmark::new();
        b.iter(move || bench.run());
    });
    group.bench_function("wide_term", |b| {
        let mut bench = iteration_term::iter_simple_wide::Benchmark::new();
        b.iter(move || bench.run());
    });

    group.bench_function("system", |b| {
        let mut bench = iteration::iter_simple_system::Benchmark::new();
        b.iter(move || bench.run());
    });
    group.bench_function("system_term", |b| {
        let mut bench = iteration_term::iter_simple_system::Benchmark::new();
        b.iter(move || bench.run());
    });

    group.bench_function("sparse_set", |b| {
        let mut bench = iteration::iter_simple_sparse_set::Benchmark::new();
        b.iter(move || bench.run());
    });
    group.bench_function("sparse_set_term", |b| {
        let mut bench = iteration_term::iter_simple_sparse_set::Benchmark::new();
        b.iter(move || bench.run());
    });

    group.bench_function("wide_sparse_set", |b| {
        let mut bench = iteration::iter_simple_wide_sparse_set::Benchmark::new();
        b.iter(move || bench.run());
    });
    group.bench_function("wide_sparse_set_term", |b| {
        let mut bench = iteration_term::iter_simple_wide_sparse_set::Benchmark::new();
        b.iter(move || bench.run());
    });

    group.finish();
}

criterion_group!(query, iter_simple);
criterion_main!(query);
