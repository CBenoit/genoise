genoise
=======

[<img alt="github" src="https://img.shields.io/badge/github-cbenoit/genoise-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/CBenoit/genoise)
[<img alt="crates.io" src="https://img.shields.io/crates/v/genoise.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/genoise)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-genoise-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/genoise)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/CBenoit/genoise/ci.yml?branch=main&style=for-the-badge" height="20">](https://github.com/CBenoit/genoise/actions?query=branch%3Amain)

*Compiler support: requires rustc 1.65+*

## What is `genoise`?

`genoise` implements [generators (a weaker, special case of coroutines)][generator] for stable Rust. 
Instead of using `#![feature(generators, generator_trait)]` and the `yield` keyword, ["extra-unstable"
features][unstable-generators] in the Rust compiler, `async`/`await` syntax is used.

Common use cases are:

- Defining iterators with self-referential states without writing `unsafe` code.
- Building state machines like it’s imperative code and leaving the compiler do the rest.

[generator]: https://en.wikipedia.org/wiki/Generator_(computer_programming)
[unstable-generators]: https://doc.rust-lang.org/nightly/unstable-book/language-features/generators.html

## What is a generator?

A generator control the flow of three types of data:

- Yield type: Each time a generator suspends execution, a value is handed to the caller.
- Resume type: Each time a generator is resumed, a value is passed in by the caller.
- Output type: When a generator completes, one final value is returned.

Here is an example taking advantage of this:

```rust
use genoise::local::{Gn, Co};
use genoise::GnState;

async fn my_generator<'a>(mut co: Co<'_, usize, bool>, input: &'a str) -> &'a str {
    let mut trimmed = input;

    while co.suspend(trimmed.len()).await {
        trimmed = &trimmed[..trimmed.len() - 1];
    }

    trimmed
}

let argument = "1234567890";
let mut generator = Gn::new(|co| my_generator(co, argument));

// A generator does nothing when created, you need to `.start()` it first
assert!(!generator.started());
assert!(matches!(generator.start(), GnState::Suspended(10)));

// Once started, you can pass in data and resume the execution using `.resume(…)`
assert!(generator.started());
assert!(matches!(generator.resume(true), GnState::Suspended(9)));
assert!(matches!(generator.resume(true), GnState::Suspended(8)));
assert!(matches!(generator.resume(false), GnState::Completed("12345678")));
```

## Why `genoise`?

- Low maintenance: `genoise` is a zero-dependency crate. There is no need to release a new version
  of `genoise` just for transitive dependencies.
- Lightweight: `genoise` consists of only a few hundred lines of code and does not rely on
  procedural macros.
- Doesn’t attempt to use reserved keywords: There are no `yield_` or `r#yield` in its API.
- Simple: You can read and grok its source code in just a few minutes.
  The most challenging part is [`GeneratorFlavor`](crate::GeneratorFlavor) which relies on GATs (Generic Associated Types).
- Continuation arguments and completion values.
- Allocation-free generators.
- Genericity over the [`GeneratorFlavor`](crate::GeneratorFlavor): Write once, use everywhere.
- No standard library: `genoise` is a no-std crate, and the `alloc` feature can be disabled.
- Not a concurrency framework nor an async runtime: `genoise` does not try to replace `tokio` or
  `smol`, and there is no platform-specific code.

## Why not `genoise`?

- You prefer an API closer to the actual generators available on Rust nightly.
- You are writing performance-sensitive code, and need to use the generator in a tight loop.
  `genoise` has not been bencharked and will probably slow down your program.
  Outside of a tight loop the cost is likely negligible.

## Flavor comparison

|                             | [`local::StackedGn`] | [`local::Gn`] | [`sync::StackedGn`] | [`sync::Gn`] |
|-----------------------------|----------------------|---------------|---------------------|--------------|
| Allocations per instance    | 0                    | 3             | 0                   | 3            |
| Can be moved                | No                   | Yes           | No                  | Yes          |
| Thread-safe (`Sync + Send`) | No                   | No            | Yes                 | yes          |

Constructing a heap-flavored generator requires three allocations:

- A memory slot for the yield value
- A memory slot for the resume value
- The `Future`-based state machine

Stack-flavored generators are using values pinned to the stack, and thus can’t be moved around.

## Unsafe usages

TODO: document this

## Relation with `Iterator`s

A generator which does not take any value when resumed nor returns any value on completion is
also an [`Iterator`](core::iter::Iterator):

```rust
use genoise::local::{Gn, Co};

async fn fibonacci(mut co: Co<'_, u32, ()>) {
    let mut a = 0;
    co.suspend(a).await;

    let mut b = 1;
    co.suspend(b).await;

    while b < 200 {
        core::mem::swap(&mut a, &mut b);
        b += a;

        co.suspend(b).await;
    }
}

let generator = Gn::new(fibonacci);
let fibonacci_sequence: Vec<u32> = generator.collect();
assert_eq!(
    fibonacci_sequence,
    [0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233]
);
```

Note that calling [`size_hint`](core::iter::Iterator::size_hint) on a generator will always return
`(0, None)` since there is no way to know how many items will be yielded by the generator.
Some generators may never terminate at all (it is advised to not call
[`collect`](core::iter::Iterator::collect) on these).

## Alternatives to `genoise`

- [genawaiter](https://crates.io/crates/genawaiter): Stackless generators on stable Rust
- [next_gen](https://crates.io/crates/next_gen): Safe generators on stable Rust
- [generator](https://crates.io/crates/generator): Stackfull Generator Library in Rust
- [remit](https://crates.io/crates/remit): Rust generators implemented through async/await syntax
- [gen-z](https://crates.io/crates/gen-z): Macro-free stream construction through asynchronous generators via an awaitable sender
- [corosensei](https://crates.io/crates/corosensei): A fast and safe implementation of stackful coroutines
- [may](https://crates.io/crates/may): Rust Stackful Coroutine Library
- [mco](https://crates.io/crates/mco): Rust Coroutine Library like go

## License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a>, <a href="LICENSE-MIT">MIT license</a> or <a href="LICENSE-ZLIB">Zlib license</a>
at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you shall be licensed as above, without any
additional terms or conditions.
</sub>