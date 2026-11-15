use std::time::{Duration, Instant};

#[inline(never)]
fn tick(counter: u64) -> u64 {
    let doubled = counter.wrapping_mul(2);
    std::hint::black_box(doubled)
}

fn main() {
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut counter = 0u64;
    while Instant::now() < deadline {
        counter = counter.wrapping_add(1);
        std::hint::black_box(tick(counter));
        std::thread::sleep(Duration::from_millis(20));
    }
}
