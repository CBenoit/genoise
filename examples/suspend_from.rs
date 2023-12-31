use library::{combine_both, do_something, do_something_else, Event, UserResponse};

const SPECIAL_CASE: &str = "rust-lang.org";

fn main() {
    {
        println!("Calling `do_something` with {:?}", SPECIAL_CASE);
        let generator = do_something(SPECIAL_CASE);
        println!("Drive the generator");
        let out = drive_generator(generator, SPECIAL_CASE, 3);
        assert_eq!(out, 3);
    }

    {
        println!("Calling `do_something_else`");
        let generator = do_something_else();
        println!("Drive the generator");
        let out = drive_generator(generator, "never exercised", 500);
        assert_eq!(out, 500);
    }

    let cases = [(SPECIAL_CASE, 3, 6), ("fallback.ninja", 500, 1000)];

    for (input, expected_payload_len, expected_out) in cases {
        println!("Calling `combine_both` with {input:?}");
        let generator = combine_both(input);
        println!("Drive the generator");
        let out = drive_generator(generator, input, expected_payload_len);
        assert_eq!(out, expected_out);
    }
}

fn drive_generator<F>(
    mut generator: genoise::Gn<'_, '_, Event<'_>, UserResponse, u32, F>,
    expected_url: &str,
    expected_payload_len: usize,
) -> u32
where
    F: genoise::GeneratorFlavor,
{
    use genoise::GnState;

    // Start the generator
    let mut state = generator.start();

    loop {
        let response = match state {
            // The generator is suspended, handle the yielded value
            GnState::Suspended(event) => {
                // How the events are actually handled is up to the caller
                // (could perform I/O with or without async)
                match event {
                    Event::HttpRequest { url } => {
                        assert_eq!(url, expected_url);
                        UserResponse::Payload(vec![1, 2, 3])
                    }
                    Event::PayloadLen(len) => {
                        assert_eq!(len, expected_payload_len);
                        UserResponse::SomeValue(u32::try_from(len).unwrap())
                    }
                }
            }
            // The generator is in its final state, break out the execution loop
            GnState::Completed(out) => break dbg!(out),
        };

        // Resume the generator
        state = generator.resume(dbg!(response));
    }
}

mod library {
    use genoise::{local, Co, GeneratorFlavor};

    // Data type our generator will yield back to the caller

    pub enum Event<'a> {
        HttpRequest { url: &'a str },
        PayloadLen(usize),
    }

    // Data type our generator will accept back at interruption points

    #[derive(Debug)]
    pub enum UserResponse {
        Payload(Vec<u8>),
        SomeValue(u32),
    }

    // The functions actually exposed to the user, simply wrapping our underlying "async" state machine

    pub fn do_something(url: &str) -> local::Gn<'_, '_, Event<'_>, UserResponse, u32> {
        local::Gn::new(|co| async { do_something_impl(co, url).await })
    }

    pub fn do_something_else<'a>() -> local::Gn<'a, 'a, Event<'a>, UserResponse, u32> {
        local::Gn::new(do_something_else_impl)
    }

    /// Combines `do_something` and `do_something_else` generators using `suspend_from`.
    pub fn combine_both(url: &str) -> local::Gn<'_, '_, Event<'_>, UserResponse, u32> {
        local::Gn::new(move |mut co| async move {
            let output = if url == crate::SPECIAL_CASE {
                co.suspend_from(do_something(url)).await
            } else {
                co.suspend_from(do_something_else()).await
            };

            output * 2
        })
    }

    // The actual code is written pretty much as usual, expect it’s actually a state
    // machine generated by the Rust compiler using async / await.
    // In this case, these functions are an implementation detail, not exposed to the user.

    async fn do_something_impl<'a, F: GeneratorFlavor>(
        mut co: Co<'_, Event<'a>, UserResponse, F>,
        url: &'a str,
    ) -> u32 {
        let user_response = co.suspend(Event::HttpRequest { url }).await;

        let UserResponse::Payload(payload) = user_response else {
            panic!("not payload")
        };

        let length = payload.len();

        let user_response = co.suspend(Event::PayloadLen(length)).await;

        let UserResponse::SomeValue(some_value) = user_response else {
            panic!("not some value")
        };

        some_value
    }

    async fn do_something_else_impl<F: GeneratorFlavor>(
        mut co: Co<'_, Event<'_>, UserResponse, F>,
    ) -> u32 {
        let user_response = co.suspend(Event::PayloadLen(500)).await;

        let UserResponse::SomeValue(some_value) = user_response else {
            panic!("not some value")
        };

        some_value
    }
}
