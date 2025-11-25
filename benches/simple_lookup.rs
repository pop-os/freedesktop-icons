use cosmic_freedesktop_icons::lookup;
use criterion::{
    AxisScale, BenchmarkId, Criterion, PlotConfiguration, criterion_group, criterion_main,
};
use std::hint::black_box;

pub fn bench_lookups(c: &mut Criterion) {
    let mut group = c.benchmark_group("ComparisonsLookups");
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    group.plot_config(plot_config);

    let args = [
        "user-home",               // (Best case) An icon that can be found in the current theme
        "firefox",                 // An icon that can be found in the hicolor default theme
        "com.valvesoftware.Steam", // An icon that resides in /usr/share/pixmaps
        "not-found",               // (Worst case) An icon that does not exist
    ];

    for arg in args {
        group.bench_with_input(
            BenchmarkId::new("freedesktop-icons-cache", arg),
            arg,
            |b, arg| {
                b.iter(|| {
                    lookup(black_box(arg))
                        .with_scale(black_box(1))
                        .with_size(black_box(24))
                        .with_theme(black_box("Adwaita"))
                        .with_cache()
                        .find()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_lookups);
criterion_main!(benches);
