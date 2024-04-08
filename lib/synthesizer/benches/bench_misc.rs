use criterion::*;

fn concat_strings(c: &mut Criterion) {
    let str1 = "hello";
    let str2 = "world";
    let mut group = c.benchmark_group("concat_strings");
    group.bench_function("concat_with_push", |b| {
        b.iter_batched(
            || (str1.to_string(), str2.to_string()),
            |(s1, s2)| {
                let mut out = String::with_capacity(s1.len() + s2.len());
                out.push_str(&s1);
                out.push_str(&s2);
                out
            },
            BatchSize::PerIteration,
        )
    });
    group.bench_function("concat_with_clone_add_str", |b| {
        b.iter_batched(
            || (str1.to_string(), str2.to_string()),
            |(s1, s2)| {
                s1.clone() + s2.as_str()
            },
            BatchSize::PerIteration,
        )
    });
    group.bench_function("concat_with_clone_and_push", |b| {
        b.iter_batched(
            || (str1.to_string(), str2.to_string()),
            |(s1, s2)| {
                let mut out = s1.clone();
                out.push_str(&s2);
                out
            },
            BatchSize::PerIteration,
        )
    });
    group.bench_function("concat_with_format", |b| {
        b.iter_batched(
            || (str1.to_string(), str2.to_string()),
            |(s1, s2)| {
                format!("{s1}{s2}")
            },
            BatchSize::PerIteration,
        )
    });

    group.finish()
}

criterion_group!(benches_misc, concat_strings);
