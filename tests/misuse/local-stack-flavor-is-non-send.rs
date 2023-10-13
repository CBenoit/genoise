use genoise::{local, GnState};

async fn my_generator(_co: local::StackCo<'_, (), ()>) {}

fn main() {
    local::let_gen!(generator, my_generator);

    std::thread::scope(|s| {
        s.spawn(|| {
            assert!(!generator.started());
            assert!(matches!(generator.start(), GnState::Completed(())));
        });
    });
}
