pub use stacked::*;

mod stacked {
    use core::{future::Future, pin::Pin};

    use crate::GeneratorFlavor;

    pub struct StackedSync;

    /// Helper to construct a stacked thread-safe generator
    #[doc(hidden)]
    #[macro_export]
    macro_rules! let_stacked_sync_gen {
        ($gn:ident, $co:ident, $fut_init:block) => {
            let yield_slot = $crate::local::new_stacked_slot();
            let resume_slot = $crate::local::new_stacked_slot();
            let $co = $crate::local::new_stacked_co(&yield_slot, &resume_slot);
            let fut = ::core::pin::pin!($fut_init);
            let mut $gn = $crate::local::StackedGn::new(&yield_slot, &resume_slot, fut);
        };
    }

    #[doc(inline)]
    pub use let_stacked_sync_gen as let_stacked_gen;

    use super::cell::{SyncRefCell, SyncRefMut};

    impl GeneratorFlavor for StackedSync {
        type Fut<'a, T> = dyn Future<Output = T> + Sync + Send + 'a
        where
            T: 'a;

        type UniquePtr<'a, T> = &'a mut T
        where
            T: ?Sized + 'a;

        type SharedPtr<'a, T> = &'a T
        where
            T: ?Sized + 'a;

        fn share<'a, T: ?Sized + 'a>(ptr: &Self::SharedPtr<'a, T>) -> Self::SharedPtr<'a, T> {
            ptr
        }

        type Borrowable<T> = SyncRefCell<T>
        where
            T: ?Sized;

        type Borrowed<'a, T> = SyncRefMut<'a, T>
        where
            T: ?Sized + 'a;

        fn borrow_mut<'a, T>(shared: &'a Self::Borrowable<T>) -> Self::Borrowed<'a, T>
        where
            T: ?Sized + 'a,
        {
            shared.borrow_mut()
        }
    }

    pub type StackedCo<'slot, Y, R> = crate::Co<'slot, Y, R, StackedSync>;

    pub type StackedGn<'gen, 'slot, Y, R, O> = crate::Gn<'gen, 'slot, Y, R, O, StackedSync>;

    pub fn new_stacked_slot<T>() -> SyncRefCell<Option<T>> {
        SyncRefCell::new(None)
    }

    pub fn new_stacked_co<'slot, Y, R>(
        yield_slot: &'slot SyncRefCell<Option<Y>>,
        resume_slot: &'slot SyncRefCell<Option<R>>,
    ) -> StackedCo<'slot, Y, R> {
        StackedCo {
            yield_slot,
            resume_slot,
        }
    }

    impl<'gen, 'slot, Y, R, O> StackedGn<'gen, 'slot, Y, R, O> {
        pub fn new(
            yield_slot: &'slot SyncRefCell<Option<Y>>,
            resume_slot: &'slot SyncRefCell<Option<R>>,
            generator: Pin<&'gen mut (dyn Future<Output = O> + Send + Sync + 'gen)>,
        ) -> Self {
            Self {
                yield_slot,
                resume_slot,
                generator,
                started: false,
            }
        }
    }
}

#[cfg(feature = "alloc")]
pub use self::allocated::*;

#[cfg(feature = "alloc")]
mod allocated {
    use alloc::boxed::Box;
    use alloc::sync::Arc;
    use core::future::Future;

    use super::cell::{SyncRefCell, SyncRefMut};
    use crate::GeneratorFlavor;

    /// Thread safe flavor, for `Send + Sync` generators
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub struct HeapSync;

    impl GeneratorFlavor for HeapSync {
        type Fut<'a, T> = dyn Future<Output = T> + Send + Sync + 'a
        where
            T: 'a;

        type UniquePtr<'a, T> = Box<T>
        where
            T: ?Sized + 'a;

        type SharedPtr<'a, T> = Arc<T>
        where
            T: ?Sized + 'a;

        fn share<'a, T>(ptr: &Self::SharedPtr<'a, T>) -> Self::SharedPtr<'a, T>
        where
            T: ?Sized + 'a,
        {
            Arc::clone(ptr)
        }

        type Borrowable<T> = SyncRefCell<T>
        where
            T: ?Sized;

        type Borrowed<'a, T> = SyncRefMut<'a, T>
        where
            T: ?Sized + 'a;

        fn borrow_mut<'a, T>(shared: &'a Self::Borrowable<T>) -> Self::Borrowed<'a, T>
        where
            T: ?Sized + 'a,
        {
            shared.borrow_mut()
        }
    }

    /// Thread safe generator controller
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type Co<'slot, Y, R> = crate::Co<'slot, Y, R, HeapSync>;

    /// Thread safe generator
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type Gn<'gen, 'slot, Y, R, O> = crate::Gn<'gen, 'slot, Y, R, O, HeapSync>;

    impl<'gen, 'slot, Y, R, O> Gn<'gen, 'slot, Y, R, O> {
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub fn new<Producer, Generator>(producer: Producer) -> Self
        where
            Producer: FnOnce(Co<'slot, Y, R>) -> Generator,
            Generator: Future<Output = O> + Send + Sync + 'gen,
        {
            let co = Co {
                yield_slot: Arc::new(SyncRefCell::new(None)),
                resume_slot: Arc::new(SyncRefCell::new(None)),
            };

            Self {
                yield_slot: Arc::clone(&co.yield_slot),
                resume_slot: Arc::clone(&co.resume_slot),
                generator: Box::pin(producer(co)),
                started: false,
            }
        }
    }
}

// Private on purpose
mod cell {
    use core::ops::DerefMut;
    use core::sync::atomic::{AtomicBool, Ordering};
    use core::{cell::UnsafeCell, ops::Deref};

    /// Thread safe equivalent of [`RefCell`](core::cell::RefCell)
    pub struct SyncRefCell<T: ?Sized> {
        lock: AtomicBool,
        cell: UnsafeCell<T>,
    }

    impl<T> SyncRefCell<T> {
        pub(crate) fn new(value: T) -> Self {
            Self {
                lock: AtomicBool::new(false),
                cell: UnsafeCell::new(value),
            }
        }
    }

    impl<T: ?Sized> SyncRefCell<T> {
        pub(crate) fn borrow_mut(&self) -> SyncRefMut<'_, T> {
            if self
                .lock
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                SyncRefMut {
                    lock: &self.lock,
                    value: unsafe {
                        // SAFETY: we ensured above that there are no references pointing to the
                        // contents of the UnsafeCell using the atomic boolean
                        &mut *self.cell.get()
                    },
                }
            } else {
                panic!("already borrowed");
            }
        }
    }

    // Same unsafe impls as `std::sync::Mutex`

    // SAFETY: SyncRefCell is Sync because we ensure there is no more than a single SyncRefMut at one time
    unsafe impl<T: ?Sized + Send> Sync for SyncRefCell<T> {}

    // SAFETY: SyncRefCell is Send because we ensure there is no more than a single SyncRefMut at one time
    unsafe impl<T: ?Sized + Send> Send for SyncRefCell<T> {}

    /// Thread safe equivalent of [`RefMut`](core::cell::RefMut)
    pub struct SyncRefMut<'a, T: ?Sized> {
        lock: &'a AtomicBool,
        value: &'a mut T,
    }

    impl<'a, T: ?Sized> Deref for SyncRefMut<'a, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            self.value
        }
    }

    impl<'a, T: ?Sized> DerefMut for SyncRefMut<'a, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.value
        }
    }

    impl<'a, T: ?Sized> Drop for SyncRefMut<'a, T> {
        fn drop(&mut self) {
            self.lock.store(false, Ordering::Release);
        }
    }
}
