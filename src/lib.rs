#![doc = include_str!("../README.md")]
#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::multiple_unsafe_ops_per_block)]
#![warn(clippy::semicolon_outside_block)]
#![warn(elided_lifetimes_in_paths)]
#![warn(unreachable_pub)]
// TODO: #![warn(missing_docs)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::fmt;
use core::future::Future;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

pub mod local;
pub mod sync;

#[macro_export]
macro_rules! let_gen {
    ($flavor:ty, $gn:ident, |$co:ident| $fut_init:block) => {
        let slot = $crate::CellSlot::default();
        let $co = $crate::Co::<_, _, $flavor>::new_stacked(&slot);
        let fut = ::core::pin::pin!($fut_init);
        let mut $gn =
            $crate::Gn::<'_, _, _, _, $flavor>::from_parts(&slot, fut as ::core::pin::Pin<&mut _>);
    };
    ($flavor:ty, $gn:ident, $fut_init:path) => {
        $crate::len_gen!($flavor, $gn, |co| { $fut_init(co) })
    };
}

/// A generator flavor
///
/// This trait is used to abstract over the inner future to be held by the generator as well as the
/// pointer families ([`UniquePtr`](Self::UniquePtr) and [`SharedPtr`](Self::SharedPtr)) and interior
/// mutability type ([`Cell`](Self::Cell) used to exchange yield and return values internally.
pub trait GeneratorFlavor {
    type Fut<'a, T: 'a>: ?Sized + Future<Output = T> + 'a;

    type UniquePtr<'a, T: 'a + ?Sized>: Deref<Target = T> + DerefMut + Unpin + 'a;

    type SharedPtr<'a, T: 'a + ?Sized>: Clone + Deref<Target = T> + Unpin + 'a;

    type Cell<T>;

    fn new_cell<T>(value: T) -> Self::Cell<T>;

    fn cell_replace<T>(cell: &Self::Cell<T>, other: T) -> T;
}

pub trait StackFlavor: GeneratorFlavor {}

pub trait HeapFlavor: GeneratorFlavor {
    fn new_shared<'a, T: 'a>(value: T) -> Self::SharedPtr<'a, T>;
}

enum Slot<Y, R> {
    Empty,
    YieldValue(Y),
    ResumeValue(R),
}

impl<Y, R> Slot<Y, R> {
    fn into_yield_value(self) -> Option<Y> {
        if let Self::YieldValue(value) = self {
            Some(value)
        } else {
            None
        }
    }

    fn into_resume_value(self) -> Option<R> {
        if let Self::ResumeValue(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

pub struct CellSlot<Y, R, F: GeneratorFlavor>(F::Cell<Slot<Y, R>>);

impl<Y, R, F: GeneratorFlavor> Default for CellSlot<Y, R, F> {
    fn default() -> Self {
        Self(F::new_cell(Slot::Empty))
    }
}

/// Used to suspend execution of a generator
///
/// "Co" stands for either _controller_ or _coroutine_.
pub struct Co<'a, Y, R, F>
where
    F: GeneratorFlavor,
    CellSlot<Y, R, F>: 'a,
{
    slot: F::SharedPtr<'a, CellSlot<Y, R, F>>,
}

impl<'a, Y, R, F: HeapFlavor> Co<'a, Y, R, F> {
    pub fn new_heap(slot: CellSlot<Y, R, F>) -> Self {
        Self {
            slot: F::new_shared(slot),
        }
    }
}

impl<'a, Y, R, F: StackFlavor> Co<'a, Y, R, F> {
    pub fn new_stacked(slot: F::SharedPtr<'a, CellSlot<Y, R, F>>) -> Self {
        Self { slot }
    }
}

impl<'a, Y, R, F> Co<'a, Y, R, F>
where
    F: GeneratorFlavor,
    Y: Unpin + 'a,
    R: 'a,
{
    /// Suspends the execution of the generator, yielding an intermediate value
    pub fn suspend(&mut self, value: Y) -> Interrupt<'a, Y, R, F> {
        Interrupt {
            yielded_value: Some(value),
            slot: F::SharedPtr::clone(&self.slot),
        }
    }

    // TODO: write a test to see what happen when a lot of "suspend" are created but not awaited
    // The expectation is that we can change the order in which values are exchanged, but no value is lost unless
    // `Interrupt` is not polled at all.

    /// Executes another generator until completion, retrieving its return value
    ///
    /// The yield and resume types of the generator must be the same as this controller, but the
    /// [flavor](GeneratorFlavor) may differ.
    pub async fn suspend_from<O, F2>(&mut self, mut generator: Gn<'a, Y, R, O, F2>) -> O
    where
        F2: GeneratorFlavor,
    {
        let mut state = generator.start();

        loop {
            let resume_value = match state {
                GnState::Suspended(yielded) => self.suspend(yielded).await,
                GnState::Completed(returned) => break returned,
            };

            state = generator.resume(resume_value);
        }
    }
}

/// Future type that resolves to the value passed in by the caller when [`Gn::resume`] is called and
/// execution is resumed.
///
/// This is the only future that may be polled by a [`Gn`].
pub struct Interrupt<'a, Y, R, F>
where
    F: GeneratorFlavor,
    CellSlot<Y, R, F>: 'a,
{
    yielded_value: Option<Y>,
    slot: F::SharedPtr<'a, CellSlot<Y, R, F>>,
}

impl<'a, Y, R, F> Future for Interrupt<'a, Y, R, F>
where
    Y: Unpin,
    F: GeneratorFlavor,
{
    type Output = R;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        if let Some(yielded_value) = this.yielded_value.take() {
            F::cell_replace(&this.slot.0, Slot::YieldValue(yielded_value));
            Poll::Pending
        } else {
            let resume_value = F::cell_replace(&this.slot.0, Slot::Empty)
                .into_resume_value()
                .expect("resume value set by generator executor");
            Poll::Ready(resume_value)
        }
    }
}

/// The result of a generator execution.
pub enum GnState<Y, O> {
    Suspended(Y),
    Completed(O),
}

impl<Y, O> fmt::Debug for GnState<Y, O>
where
    Y: fmt::Debug,
    O: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GnState::Suspended(yielded) => write!(f, "Suspended({yielded:?})"),
            GnState::Completed(returned) => write!(f, "Completed({returned:?})"),
        }
    }
}

/// A generator
///
/// Generators control the flow of three types of data:
///
/// - Yield type: Each time a generator suspends execution, a value is handed to the caller.
/// - Resume type: Each time a generator is resumed, a value is passed in by the caller.
/// - Output type: When a generator completes, one final value is returned.
#[must_use = "generators do nothing unless you `.start()` or `.resume(…)` them"]
pub struct Gn<'a, Y, R, O, F>
where
    O: 'a,
    CellSlot<Y, R, F>: 'a,
    F: GeneratorFlavor,
{
    slot: F::SharedPtr<'a, CellSlot<Y, R, F>>,
    generator: Pin<F::UniquePtr<'a, F::Fut<'a, O>>>,
    started: bool,
}

impl<'a, Y, R, O, F: GeneratorFlavor> Gn<'a, Y, R, O, F> {
    pub fn from_parts(
        slot: F::SharedPtr<'a, CellSlot<Y, R, F>>,
        generator: Pin<F::UniquePtr<'a, F::Fut<'a, O>>>,
    ) -> Self {
        Self {
            slot,
            generator,
            started: false,
        }
    }

    /// Returns whether the generator was started or not
    pub fn started(&self) -> bool {
        self.started
    }

    /// Starts execution of the generator
    ///
    /// This method must be called exactly once before calling [`resume`](Self::resume).
    pub fn start(&mut self) -> GnState<Y, O> {
        self.started = true;
        self.step()
    }

    /// Resumes execution of the generator, passing in a value
    ///
    /// [`start`](Self::start) must be called before resumption can happen.
    pub fn resume(&mut self, value: R) -> GnState<Y, O> {
        assert!(
            self.started,
            "generator must be started before it can be resumed"
        );

        F::cell_replace(&self.slot.0, Slot::ResumeValue(value));

        self.step()
    }

    fn step(&mut self) -> GnState<Y, O> {
        match execute_one_step(self.generator.as_mut()) {
            None => {
                let value = F::cell_replace(&self.slot.0, Slot::Empty)
                    .into_yield_value()
                    .expect("yielded value set by the `await`ed `Interrupt`");
                GnState::Suspended(value)
            }
            Some(value) => GnState::Completed(value),
        }
    }
}

#[must_use]
fn noop_waker() -> Waker {
    use core::task::{RawWaker, RawWakerVTable};

    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        // Cloning just returns a new no-op raw waker
        |_| RAW,
        // `wake` does nothing
        |_| {},
        // `wake_by_ref` does nothing
        |_| {},
        // Dropping does nothing as we don't allocate anything
        |_| {},
    );
    const RAW: RawWaker = RawWaker::new(core::ptr::null(), &VTABLE);

    // SAFETY: the contract defined RawWaker's and RawWakerVTable's documentation is upheld, see above
    unsafe { Waker::from_raw(RAW) }
}

fn execute_one_step<F: Future + ?Sized>(generator: Pin<&mut F>) -> Option<F::Output> {
    // TODO: use Waker::noop when stabilized
    // https://doc.rust-lang.org/std/task/struct.Waker.html#method.noop
    let noop_waker = noop_waker();

    let mut context = Context::from_waker(&noop_waker);

    match generator.poll(&mut context) {
        Poll::Pending => None,
        Poll::Ready(item) => Some(item),
    }
}

impl<'a, Y, F> Iterator for Gn<'a, Y, (), (), F>
where
    F: GeneratorFlavor,
{
    type Item = Y;

    fn next(&mut self) -> Option<Self::Item> {
        let state = if self.started {
            self.resume(())
        } else {
            self.start()
        };

        match state {
            GnState::Suspended(value) => Some(value),
            GnState::Completed(()) => None,
        }
    }
}
