use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

const INPUTS: &[(&str, &[u8])] = &[
    (
        "moordread",
        include_bytes!("../builds/Castle_MoorDread_-_FFA.brs"),
    ),
    ("city", include_bytes!("../builds/Brickadia_City.brs")),
    ("legfast_qa", include_bytes!("../builds/LEGFAST_QA.brs")),
    (
        "halloweenfest",
        include_bytes!("../builds/HalloweenFest_Spirit.brs"),
    ),
    ("lego_island", include_bytes!("../builds/Lego_Island_2.brs")),
    ("skidrow", include_bytes!("../builds/skidrow.brs")),
    ("h00gle", include_bytes!("../builds/h00gle.brs")),
];

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("read");
    group.measurement_time(std::time::Duration::from_secs(30));
    group.sample_size(15);
    for (name, data) in INPUTS.iter() {
        let bricks = brick_count(data);
        group.throughput(Throughput::Elements(bricks as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &data, |b, i| {
            b.iter(|| read_through(i))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("rewrite");
    group.measurement_time(std::time::Duration::from_secs(60));
    group.sample_size(10);
    for (name, data) in INPUTS.iter() {
        let bricks = brick_count(data);
        group.throughput(Throughput::Elements(bricks as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &data, |b, i| {
            b.iter(|| rewrite(i))
        });
    }
    group.finish();
}

fn brick_count(data: &[u8]) -> u32 {
    let reader = brs::Reader::new(data).unwrap();
    reader.brick_count()
}

fn read_through(buf: &[u8]) {
    let reader = brs::Reader::new(buf).unwrap();
    // Optional: let (reader, screenshot) = reader.screenshot_data().unwrap();
    let (reader, bricks) = reader.bricks().unwrap();
    black_box(bricks.collect::<Result<Vec<_>, _>>().unwrap());
    let (_reader, components) = reader.components().unwrap();
    black_box(components.iter().collect::<Vec<_>>());
}

fn rewrite(buf: &[u8]) {
    let reader = brs::Reader::new(buf).unwrap();
    let write_data = reader.into_write_data().unwrap();
    brs::write_save(&mut Null, &write_data).unwrap();
}

struct Null;

impl std::io::Write for Null {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        black_box(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
