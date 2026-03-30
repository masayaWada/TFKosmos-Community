//! スキャン並列化のパフォーマンスベンチマーク
//!
//! `cargo bench` で実行

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// serde_json::Map のマージ性能を計測
fn bench_map_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_merge");

    group.bench_function("merge_7_small_maps", |b| {
        b.iter(|| {
            let mut base = Map::new();
            for i in 0..7 {
                let mut partial = Map::new();
                for j in 0..3 {
                    partial.insert(
                        format!("resource_{}_{}", i, j),
                        Value::Array(vec![json!({"id": format!("id-{}-{}", i, j)})]),
                    );
                }
                base.extend(black_box(partial));
            }
            base
        })
    });

    group.bench_function("merge_7_large_maps", |b| {
        let maps: Vec<Map<String, Value>> = (0..7)
            .map(|i| {
                let mut m = Map::new();
                for j in 0..100 {
                    let items: Vec<Value> = (0..10)
                        .map(|k| json!({"id": format!("id-{}-{}-{}", i, j, k), "name": "test"}))
                        .collect();
                    m.insert(format!("resource_{}_{}", i, j), Value::Array(items));
                }
                m
            })
            .collect();

        b.iter(|| {
            let mut base = Map::new();
            for map in &maps {
                base.extend(black_box(map.clone()));
            }
            base
        })
    });

    group.finish();
}

/// AtomicUsize プログレスカウンターのオーバーヘッド計測
fn bench_atomic_progress(c: &mut Criterion) {
    let mut group = c.benchmark_group("atomic_progress");

    group.bench_function("fetch_add_and_load_100", |b| {
        let counter = Arc::new(AtomicUsize::new(0));
        b.iter(|| {
            counter.store(0, Ordering::Relaxed);
            for _ in 0..100 {
                counter.fetch_add(1, Ordering::Relaxed);
                let current = counter.load(Ordering::Relaxed);
                black_box(current * 100 / 100);
            }
        })
    });

    group.bench_function("concurrent_fetch_add_7_threads", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            let counter = Arc::new(AtomicUsize::new(0));
            rt.block_on(async {
                let handles: Vec<_> = (0..7)
                    .map(|_| {
                        let c = counter.clone();
                        tokio::spawn(async move {
                            for _ in 0..10 {
                                c.fetch_add(1, Ordering::Relaxed);
                                let current = c.load(Ordering::Relaxed);
                                black_box(current * 100 / 70);
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.await.unwrap();
                }
            });
            assert_eq!(counter.load(Ordering::Relaxed), 70);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_map_merge, bench_atomic_progress);
criterion_main!(benches);
