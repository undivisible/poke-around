use criterion::{Criterion, black_box, criterion_group, criterion_main};

// We will mock a simplified loop with a cloneable dummy state
#[derive(Clone)]
struct DummyState {
    // some data
    #[allow(dead_code)]
    data: std::sync::Arc<Vec<u8>>,
}

fn handle_with_clone(items: &[u32], state: DummyState) {
    for item in items {
        let cloned_state = state.clone();
        std::hint::black_box(process_item(item, cloned_state));
    }
}

fn handle_with_borrow(items: &[u32], state: &DummyState) {
    for item in items {
        std::hint::black_box(process_item_borrow(item, state));
    }
}

fn process_item(_item: &u32, _state: DummyState) -> u32 {
    1
}

fn process_item_borrow(_item: &u32, _state: &DummyState) -> u32 {
    1
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let items: Vec<u32> = (0..100).collect();
    let state = DummyState {
        data: std::sync::Arc::new(vec![0; 100]),
    };

    c.bench_function("loop_with_clone", |b| {
        b.iter(|| handle_with_clone(black_box(&items), black_box(state.clone())))
    });

    c.bench_function("loop_with_borrow", |b| {
        b.iter(|| handle_with_borrow(black_box(&items), black_box(&state)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
