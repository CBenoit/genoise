pub use stacked::*;

mod stacked {
    use core::{
        cell::{RefCell, RefMut},
        future::Future,
        pin::Pin,
    };

    use crate::GeneratorFlavor;

    pub struct StackedLocal;

    /// Helper to construct a stacked local generator
    #[doc(hidden)]
    #[macro_export]
    macro_rules! let_stacked_local_gen {
        ($gn:ident, $co:ident, $fut_init:block) => {
            let yield_slot = $crate::local::new_stacked_slot();
            let resume_slot = $crate::local::new_stacked_slot();
            let $co = $crate::local::new_stacked_co(&yield_slot, &resume_slot);
            let fut = ::core::pin::pin!($fut_init);
            // TODO: check what happen when mutability is not used (warning?)
            let mut $gn = $crate::local::StackedGn::new(&yield_slot, &resume_slot, fut);
        };
    }

    #[doc(inline)]
    pub use let_stacked_local_gen as let_stacked_gen;

    impl GeneratorFlavor for StackedLocal {
        type Fut<'a, T> = dyn Future<Output = T> + 'a
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

        type Borrowable<T> = RefCell<T>
        where
            T: ?Sized;

        type Borrowed<'a, T> = RefMut<'a, T>
        where
            T: ?Sized + 'a;

        fn borrow_mut<'a, T>(shared: &'a Self::Borrowable<T>) -> Self::Borrowed<'a, T>
        where
            T: ?Sized + 'a,
        {
            shared.borrow_mut()
        }
    }

    pub type StackedCo<'slot, Y, R> = crate::Co<'slot, Y, R, StackedLocal>;

    pub type StackedGn<'gen, 'slot, Y, R, O> = crate::Gn<'gen, 'slot, Y, R, O, StackedLocal>;

    pub fn new_stacked_slot<T>() -> RefCell<Option<T>> {
        RefCell::new(None)
    }

    pub fn new_stacked_co<'slot, Y, R>(
        yield_slot: &'slot RefCell<Option<Y>>,
        resume_slot: &'slot RefCell<Option<R>>,
    ) -> StackedCo<'slot, Y, R> {
        StackedCo {
            yield_slot,
            resume_slot,
        }
    }

    impl<'gen, 'slot, Y, R, O> StackedGn<'gen, 'slot, Y, R, O> {
        pub fn new(
            yield_slot: &'slot RefCell<Option<Y>>,
            resume_slot: &'slot RefCell<Option<R>>,
            generator: Pin<&'gen mut (dyn Future<Output = O> + 'gen)>,
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
pub use self::heap::*;

#[cfg(feature = "alloc")]
mod heap {
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use core::cell::{RefCell, RefMut};
    use core::future::Future;

    use crate::GeneratorFlavor;

    /// Thread local flavor, for non-`Send + Sync` generators
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub struct HeapLocal;

    impl GeneratorFlavor for HeapLocal {
        type Fut<'a, T> = dyn Future<Output = T> + 'a
        where
            T: 'a;

        type UniquePtr<'a, T> = Box<T>
        where
            T: ?Sized + 'a;

        type SharedPtr<'a, T> = Rc<T>
        where
            T: ?Sized + 'a;

        fn share<'a, T>(ptr: &Self::SharedPtr<'a, T>) -> Self::SharedPtr<'a, T>
        where
            T: ?Sized + 'a,
        {
            Rc::clone(ptr)
        }

        type Borrowable<T> = RefCell<T>
        where
            T: ?Sized;

        type Borrowed<'a, T> = RefMut<'a, T>
        where
            T: ?Sized + 'a;

        fn borrow_mut<'a, T>(shared: &'a Self::Borrowable<T>) -> Self::Borrowed<'a, T>
        where
            T: ?Sized + 'a,
        {
            shared.borrow_mut()
        }
    }

    /// Thread local generator controller
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type Co<'slot, Y, R> = crate::Co<'slot, Y, R, HeapLocal>;

    /// Thread local generator
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type Gn<'gen, 'slot, Y, R, O> = crate::Gn<'gen, 'slot, Y, R, O, HeapLocal>;

    impl<'gen, 'slot, Y, R, O> Gn<'gen, 'slot, Y, R, O> {
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub fn new<Producer, Generator>(producer: Producer) -> Self
        where
            Producer: FnOnce(Co<'slot, Y, R>) -> Generator,
            Generator: Future<Output = O> + 'gen,
        {
            let co = Co {
                yield_slot: Rc::new(RefCell::new(None)),
                resume_slot: Rc::new(RefCell::new(None)),
            };

            Self {
                yield_slot: Rc::clone(&co.yield_slot),
                resume_slot: Rc::clone(&co.resume_slot),
                generator: Box::pin(producer(co)),
                started: false,
            }
        }
    }
}
