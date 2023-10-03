use genoise::{local, sync, Co, GeneratorFlavor, Gn, GnState};

fn check_generator<F: GeneratorFlavor>(g: &mut Gn<'_, String, usize, bool, F>) {
    if let GnState::Suspended(value) = g.start() {
        assert!(matches!(g.resume(value.len()), GnState::Completed(true)));
    } else {
        panic!()
    }
}

async fn generator<F: GeneratorFlavor>(mut co: Co<'_, String, usize, F>) -> bool {
    let initial = "hello".to_owned();
    let initial_len = initial.len();
    let len = co.suspend(initial).await;
    len == initial_len
}

#[test]
fn check_sync_gn_can_be_returned() {
    fn produce_a_generator() -> sync::StaticGn<String, usize, bool> {
        sync::Gn::new(generator)
    }

    let mut g = produce_a_generator();
    check_generator(&mut g);
}

#[test]
fn check_local_gn_can_be_returned() {
    fn produce_a_generator() -> local::StaticGn<String, usize, bool> {
        local::Gn::new(generator)
    }

    let mut g = produce_a_generator();
    check_generator(&mut g);
}

async fn generator_taking_ref<F: GeneratorFlavor>(
    mut co: Co<'_, String, usize, F>,
    input: &str,
) -> bool {
    let initial = input.to_owned();
    let initial_len = initial.len();
    let len = co.suspend(initial).await;
    len == initial_len
}

#[test]
fn check_sync_owned_gn_taking_ref_can_be_returned() {
    fn produce_a_generator() -> sync::StaticGn<String, usize, bool> {
        sync::Gn::new(|co| generator_taking_ref(co, "hello"))
    }

    let mut g = produce_a_generator();
    check_generator(&mut g);
}

#[test]
fn check_local_owned_gn_taking_ref_can_be_returned() {
    fn produce_a_generator() -> local::StaticGn<String, usize, bool> {
        local::Gn::new(|co| generator_taking_ref(co, "hello"))
    }

    let mut g = produce_a_generator();
    check_generator(&mut g);
}

async fn generator_yielding_ref<'a, F: GeneratorFlavor>(
    mut co: Co<'_, &'a str, usize, F>,
    input: &'a str,
) -> bool {
    let len = co.suspend(input).await;
    len == input.len()
}

fn check_generator_yielding_ref<F: GeneratorFlavor>(g: &mut Gn<'_, &str, usize, bool, F>) {
    if let GnState::Suspended(value) = g.start() {
        assert!(matches!(g.resume(value.len()), GnState::Completed(true)));
    } else {
        panic!()
    }
}

// FIXME: higher-ranked lifetime error
// #[test]
// fn check_sync_owned_gn_yielding_ref_can_be_returned() {
//     fn produce_a_generator(input: &str) -> sync::Gn<'_, '_, &str, usize, bool> {
//         sync::Gn::new(|co| generator_yielding_ref(co, input))
//     }

//     let input = String::from("hello");
//     let mut g = produce_a_generator(&input);
//     check_generator_yielding_ref(&mut g);
// }

#[test]
fn check_local_owned_gn_yielding_ref_can_be_returned() {
    fn produce_a_generator(input: &str) -> local::Gn<'_, &str, usize, bool> {
        local::Gn::new(|co| generator_yielding_ref(co, input))
    }

    let input = String::from("hello");
    let mut g = produce_a_generator(&input);
    check_generator_yielding_ref(&mut g);
}

#[test]
fn check_hardcoded_local_owned_gn() {
    async fn local_generator<'a>(mut co: local::Co<'_, &'a str, usize>, input: &'a str) -> bool {
        let len = co.suspend(input).await;
        len == input.len()
    }

    fn produce_a_generator(input: &str) -> local::Gn<'_, &str, usize, bool> {
        local::Gn::new(|co| local_generator(co, input))
    }

    let input = String::from("hello");
    let mut g = produce_a_generator(&input);
    check_generator_yielding_ref(&mut g);
}
