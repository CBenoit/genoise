#![doc = include_str!("../README.md")]
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

/// A generator flavor
///
/// This trait is used to abstract over the inner future to be held by the generator as well as the
/// pointer families ([`UniquePtr`](Self::UniquePtr) and [`SharedPtr`](Self::SharedPtr)) and interior
/// mutability types ([`Borrowable`](Self::Borrowable) and [`Borrowed`](Self::Borrowed)) used to
/// exchange yield and return values internally.
pub trait GeneratorFlavor {
    type Fut<'a, T>: ?Sized + Future<Output = T> + 'a
    where
        T: 'a;

    type UniquePtr<'a, T>: Deref<Target = T> + DerefMut + Unpin
    where
        T: ?Sized + 'a;

    type SharedPtr<'a, T>: Deref<Target = T> + Unpin
    where
        T: ?Sized + 'a;

    fn share<'a, T>(ptr: &Self::SharedPtr<'a, T>) -> Self::SharedPtr<'a, T>
    where
        T: ?Sized + 'a;

    type Borrowable<T>: ?Sized
    where
        T: ?Sized;

    type Borrowed<'a, T>: Deref<Target = T> + DerefMut + 'a
    where
        T: ?Sized + 'a;

    fn borrow_mut<'a, T>(shared: &'a Self::Borrowable<T>) -> Self::Borrowed<'a, T>
    where
        T: ?Sized + 'a;
}

/// Future type that resolves to the value passed in by the caller when [`Gn::resume`] is called and
/// execution is resumed.
///
/// This is the only future that may be polled by a [`Gn`].
pub struct Interrupt<'slot, Y, R, F>
where
    F: GeneratorFlavor,
    F::Borrowable<Option<Y>>: 'slot,
    F::Borrowable<Option<R>>: 'slot,
{
    yielded_value: Option<Y>,
    yield_slot: F::SharedPtr<'slot, F::Borrowable<Option<Y>>>,
    resume_slot: F::SharedPtr<'slot, F::Borrowable<Option<R>>>,
}

impl<'slot, Y, R, F> Future for Interrupt<'slot, Y, R, F>
where
    Y: Unpin,
    F: GeneratorFlavor,
{
    type Output = R;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        if let Some(yielded_value) = this.yielded_value.take() {
            *F::borrow_mut(&this.yield_slot) = Some(yielded_value);
            Poll::Pending
        } else {
            let resume_value = F::borrow_mut(&this.resume_slot)
                .take()
                .expect("resume value set by generator executor");
            Poll::Ready(resume_value)
        }
    }
}

/// Used to suspend execution of a generator
///
/// "Co" stands for either _controller_ or _coroutine_.
pub struct Co<'slot, Y, R, F>
where
    F: GeneratorFlavor,
    F::Borrowable<Option<Y>>: 'slot,
    F::Borrowable<Option<R>>: 'slot,
{
    yield_slot: F::SharedPtr<'slot, F::Borrowable<Option<Y>>>,
    resume_slot: F::SharedPtr<'slot, F::Borrowable<Option<R>>>,
}

impl<'slot, Y, R, F> Co<'slot, Y, R, F>
where
    F: GeneratorFlavor,
    Y: Unpin + 'slot,
    R: 'slot,
{
    /// Suspends the execution of the generator, yielding an intermediate value
    pub fn suspend(&mut self, value: Y) -> Interrupt<'slot, Y, R, F> {
        Interrupt {
            yielded_value: Some(value),
            yield_slot: F::share(&self.yield_slot),
            resume_slot: F::share(&self.resume_slot),
        }
    }

    /// Executes another generator until completion, retrieving its return value
    ///
    /// The yield and resume types of the generator must be the same as this controller, but the
    /// [flavor](GeneratorFlavor) may differ.
    pub async fn suspend_from<'gen, O, F2>(
        &mut self,
        mut generator: Gn<'gen, 'slot, Y, R, O, F2>,
    ) -> O
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

/// The result of a generator execution.
#[must_use]
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
pub struct Gn<'gen, 'slot, Y, R, O, F>
where
    O: 'gen,
    F::Borrowable<Option<Y>>: 'slot,
    F::Borrowable<Option<R>>: 'slot,
    F: GeneratorFlavor,
{
    yield_slot: F::SharedPtr<'slot, F::Borrowable<Option<Y>>>,
    resume_slot: F::SharedPtr<'slot, F::Borrowable<Option<R>>>,
    generator: Pin<F::UniquePtr<'gen, F::Fut<'gen, O>>>,
    started: bool,
}

impl<'gen, 'slot, Y, R, O, F> Gn<'gen, 'slot, Y, R, O, F>
where
    F: GeneratorFlavor,
{
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

        *F::borrow_mut(&self.resume_slot) = Some(value);

        self.step()
    }

    fn step(&mut self) -> GnState<Y, O> {
        match execute_one_step(self.generator.as_mut()) {
            None => {
                let value = F::borrow_mut(&self.yield_slot)
                    .take()
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

    unsafe {
        // SAFETY: the contract defined RawWaker's and RawWakerVTable's documentation is upheld, see above
        Waker::from_raw(RAW)
    }
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

impl<'gen, 'slot, Y, F> Iterator for Gn<'gen, 'slot, Y, (), (), F>
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
