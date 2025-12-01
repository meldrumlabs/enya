use criterion::{Criterion, criterion_group, criterion_main};
use std::sync::Arc;
use talna_v2::object_store::local::LocalFileSystem;
use talna_v2::{MetricName, tagset};

/// Helper to run async code in benchmarks
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn intersection(c: &mut Criterion) {
    let v = vec![vec![1, 2, 3, 4, 5], vec![1, 3, 5], vec![1, 3]];

    c.bench_function("intersection", |b| {
        b.iter(|| talna_v2::query::filter::intersection(&v));
    });
}

fn union(c: &mut Criterion) {
    c.bench_function("union (short)", |b| {
        let v = vec![vec![1, 2, 3, 4, 5], vec![1, 3, 5], vec![1, 3]];
        b.iter(|| talna_v2::query::filter::union(&v));
    });

    c.bench_function("union (long)", |b| {
        let v = vec![
            (0..100).collect(),
            (50..200).collect(),
            vec![1, 3],
            vec![1, 3],
            vec![5, 7],
            vec![4, 5],
            vec![8, 9],
            vec![9, 10],
        ];
        b.iter(|| talna_v2::query::filter::union(&v));
    });
}

fn join_tags(c: &mut Criterion) {
    let tags = tagset!(
      "service" => "db",
      "env" => "prod",
      "host" => "host-1",
    );

    c.bench_function("join tags", |b| {
        b.iter(|| {
            let mut str = talna_v2::SeriesKey::allocate_string_for_tags(tags, 0);
            talna_v2::SeriesKey::join_tags(&mut str, tags);
        });
    });
}

fn create_series_key(c: &mut Criterion) {
    let metric_name = MetricName::try_from("cpu.total").unwrap();

    let tags = tagset!(
      "service" => "db",
      "env" => "prod",
      "host" => "host-1",
    );

    c.bench_function("create series key", |b| {
        b.iter(|| {
            let _ = talna_v2::SeriesKey::format(metric_name, tags);
        });
    });
}

fn parse_filter_query(c: &mut Criterion) {
    c.bench_function("parse filter query (simple)", |b| {
        b.iter(|| {
            talna_v2::query::filter::parse_filter_query("service:db AND env:prod").unwrap();
        });
    });

    c.bench_function("parse filter query (complex)", |b| {
        b.iter(|| {
            talna_v2::query::filter::parse_filter_query(
                "os:debian AND service:db AND (env:prod OR env:staging)",
            )
            .unwrap();
        });
    });
}

fn insert_timestamp(c: &mut Criterion) {
    let metric_name = MetricName::try_from("cpu").unwrap();
    let rt = runtime();

    c.bench_function("insert single", |b| {
        let tags = tagset!(
            "service" => "db",
            "env" => "prod",
            "host" => "host-1",
        );

        let dir = tempfile::tempdir().unwrap();
        let object_store = Arc::new(
            LocalFileSystem::new_with_prefix(dir.path()).expect("failed to create object store"),
        );

        let db = rt
            .block_on(talna_v2::Database::builder().open(object_store, "bench-db"))
            .unwrap();

        let mut ts = 0u128;

        b.iter(|| {
            rt.block_on(db.write_at(metric_name, ts, 52.74, tags))
                .unwrap();
            ts += 1;
        });

        rt.block_on(db.close()).unwrap();
    });
}

fn avg(c: &mut Criterion) {
    let metric_name = MetricName::try_from("cpu").unwrap();
    let rt = runtime();

    c.bench_function("avg", |b| {
        let dir = tempfile::tempdir().unwrap();
        let object_store = Arc::new(
            LocalFileSystem::new_with_prefix(dir.path()).expect("failed to create object store"),
        );

        let db = rt
            .block_on(talna_v2::Database::builder().open(object_store, "bench-db"))
            .unwrap();

        let tags = tagset!(
            "service" => "db",
            "env" => "prod",
            "host" => "host-1",
        );

        rt.block_on(db.write(metric_name, 10.0, tags)).unwrap();
        rt.block_on(db.write(metric_name, 11.0, tags)).unwrap();
        rt.block_on(db.write(metric_name, 12.0, tags)).unwrap();
        rt.block_on(db.write(metric_name, 13.0, tags)).unwrap();
        rt.block_on(db.write(metric_name, 14.0, tags)).unwrap();

        b.iter(|| {
            rt.block_on(async {
                db.avg(metric_name, "host")
                    .filter("service:db AND env:prod")
                    .build()
                    .await
                    .unwrap()
                    .collect()
                    .unwrap()
            })
        });

        rt.block_on(db.close()).unwrap();
    });

    c.bench_function("avg (multi series)", |b| {
        let dir = tempfile::tempdir().unwrap();
        let object_store = Arc::new(
            LocalFileSystem::new_with_prefix(dir.path()).expect("failed to create object store"),
        );

        let db = rt
            .block_on(talna_v2::Database::builder().open(object_store, "bench-db"))
            .unwrap();

        rt.block_on(async {
            {
                let tags = tagset!(
                    "service" => "db",
                    "env" => "prod",
                    "host" => "host-1",
                );

                db.write(metric_name, 10.0, tags).await.unwrap();
                db.write(metric_name, 11.0, tags).await.unwrap();
                db.write(metric_name, 12.0, tags).await.unwrap();
                db.write(metric_name, 13.0, tags).await.unwrap();
                db.write(metric_name, 14.0, tags).await.unwrap();
            }
            {
                let tags = tagset!(
                    "service" => "db",
                    "env" => "prod",
                    "host" => "host-2",
                );

                db.write(metric_name, 10.0, tags).await.unwrap();
                db.write(metric_name, 11.0, tags).await.unwrap();
                db.write(metric_name, 12.0, tags).await.unwrap();
                db.write(metric_name, 13.0, tags).await.unwrap();
                db.write(metric_name, 14.0, tags).await.unwrap();
            }
            {
                let tags = tagset!(
                    "service" => "db",
                    "env" => "prod",
                    "host" => "host-3",
                );

                db.write(metric_name, 10.0, tags).await.unwrap();
                db.write(metric_name, 11.0, tags).await.unwrap();
                db.write(metric_name, 12.0, tags).await.unwrap();
                db.write(metric_name, 13.0, tags).await.unwrap();
                db.write(metric_name, 14.0, tags).await.unwrap();
            }
            {
                let tags = tagset!(
                    "service" => "ui",
                    "env" => "prod",
                    "host" => "host-3",
                );

                db.write(metric_name, 10.0, tags).await.unwrap();
                db.write(metric_name, 11.0, tags).await.unwrap();
                db.write(metric_name, 12.0, tags).await.unwrap();
                db.write(metric_name, 13.0, tags).await.unwrap();
                db.write(metric_name, 14.0, tags).await.unwrap();
            }
        });

        b.iter(|| {
            rt.block_on(async {
                db.avg(metric_name, "host")
                    .filter("service:db AND env:prod")
                    .build()
                    .await
                    .unwrap()
                    .collect()
                    .unwrap()
            })
        });

        rt.block_on(db.close()).unwrap();
    });
}

criterion_group!(
    benches,
    intersection,
    union,
    create_series_key,
    join_tags,
    parse_filter_query,
    insert_timestamp,
    avg,
);
criterion_main!(benches);
