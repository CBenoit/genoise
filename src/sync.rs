pub use stack::*;

mod stack {
    use core::{future::Future, pin::Pin};

    use crate::{GeneratorFlavor, StackFlavor};

    pub struct StackSync;

    /// Helper to construct a stacked thread-safe generator
    #[doc(hidden)]
    #[macro_export]
    macro_rules! let_sync_gen {
        ($gn:ident, |$co:ident| $fut_init:block) => {
            $crate::let_gen!($crate::sync::StackSync, $gn, |$co| $fut_init)
        };
        ($gn:ident, $fut_init:path) => {
            $crate::let_gen!($crate::sync::StackSync, $gn, |co| { $fut_init(co) })
        };
    }

    #[doc(inline)]
    pub use let_sync_gen as let_gen;

    use super::cell::SyncRefCell;

    impl GeneratorFlavor for StackSync {
        type Fut<'a, T: 'a> = dyn Future<Output = T> + Send + 'a;

        type UniquePtr<'a, T: ?Sized + 'a> = &'a mut T;

        type SharedPtr<'a, T: ?Sized + 'a> = &'a T;

        type Cell<T> = SyncRefCell<T>;

        fn new_cell<T>(value: T) -> Self::Cell<T> {
            SyncRefCell::new(value)
        }

        #[track_caller]
        fn cell_replace<T>(cell: &Self::Cell<T>, other: T) -> T {
            cell.replace(other)
        }
    }

    impl StackFlavor for StackSync {}

    pub type StackCellSlot<Y, R> = crate::CellSlot<Y, R, StackSync>;

    pub type StackCo<'a, Y, R> = crate::Co<'a, Y, R, StackSync>;

    pub type StackGn<'a, Y, R, O> = crate::Gn<'a, Y, R, O, StackSync>;

    impl<'a, Y, R, O> StackGn<'a, Y, R, O> {
        pub fn new(
            slot: &'a StackCellSlot<Y, R>,
            generator: Pin<&'a mut (dyn Future<Output = O> + Send + 'a)>,
        ) -> Self {
            Self {
                slot,
                generator,
                started: false,
            }
        }
    }
}

#[cfg(feature = "alloc")]
pub use self::heap::*;

#[cfg(feature = "alloc")]
mod heap {
    use alloc::boxed::Box;
    use alloc::sync::Arc;
    use core::future::Future;

    use super::cell::SyncRefCell;
    use crate::{CellSlot, GeneratorFlavor, HeapFlavor};

    /// Thread safe flavor, for `Send + Sync` generators
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub struct HeapSync;

    impl GeneratorFlavor for HeapSync {
        type Fut<'a, T: 'a> = dyn Future<Output = T> + Send + 'a;

        type UniquePtr<'a, T: ?Sized + 'a> = Box<T>;

        type SharedPtr<'a, T: ?Sized + 'a> = Arc<T>;

        type Cell<T> = SyncRefCell<T>;

        fn new_cell<T>(value: T) -> Self::Cell<T> {
            SyncRefCell::new(value)
        }

        #[track_caller]
        fn cell_replace<T>(cell: &Self::Cell<T>, other: T) -> T {
            cell.replace(other)
        }
    }

    impl HeapFlavor for HeapSync {
        fn new_shared<'a, T: 'a>(value: T) -> Self::SharedPtr<'a, T> {
            Arc::new(value)
        }
    }

    /// Thread safe generator controller
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type Co<'a, Y, R> = crate::Co<'a, Y, R, HeapSync>;

    /// Thread safe generator controller holding items with 'static lifetime only
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type StaticCo<Y, R> = Co<'static, Y, R>;

    /// Thread safe generator
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type Gn<'a, Y, R, O> = crate::Gn<'a, Y, R, O, HeapSync>;

    impl<'a, Y, R, O> Gn<'a, Y, R, O> {
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub fn new<Producer, Generator>(producer: Producer) -> Self
        where
            Producer: FnOnce(Co<'a, Y, R>) -> Generator,
            Generator: Future<Output = O> + Send + 'a,
        {
            let co = Co::new_heap(CellSlot::default());
            let slots = Arc::clone(&co.slot);
            let generator = Box::pin(producer(co));
            Self::from_parts(slots, generator)
        }
    }

    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type StaticGn<Y, R, O> = crate::Gn<'static, Y, R, O, HeapSync>;
}

// NOTE: This module is private on purpose. The `SyncRefCell` type is not part of the public API.
#[allow(unreachable_pub)]
mod cell {
    use core::ops::DerefMut;
    use core::sync::atomic::{AtomicBool, Ordering};
    use core::{cell::UnsafeCell, ops::Deref};

    /// Synchronized counterpart to [`RefCell`](core::cell::RefCell)
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

    impl<T> SyncRefCell<T> {
        #[track_caller]
        pub(crate) fn replace(&self, other: T) -> T {
            core::mem::replace(&mut *self.borrow_mut(), other)
        }
    }

    impl<T: ?Sized> SyncRefCell<T> {
        #[track_caller]
        pub(crate) fn borrow_mut(&self) -> SyncRefMut<'_, T> {
            if self
                .lock
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                SyncRefMut {
                    lock: &self.lock,
                    // SAFETY: using the atomic boolean, we ensured above that there are no other
                    // references pointing to the contents of the UnsafeCell
                    value: unsafe { &mut *self.cell.get() },
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
