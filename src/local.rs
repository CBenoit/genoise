pub use stacked::*;

mod stacked {
    use core::{cell::Cell, future::Future, pin::Pin};

    use crate::{GeneratorFlavor, StackFlavor};

    pub struct StackLocal;

    /// Helper to construct a stacked local generator
    #[doc(hidden)]
    #[macro_export]
    macro_rules! let_local_gen {
        ($gn:ident, |$co:ident| $fut_init:block) => {
            $crate::let_gen!($crate::local::StackLocal, $gn, |$co| $fut_init)
        };
        ($gn:ident, $fut_init:path) => {
            $crate::let_gen!($crate::local::StackLocal, $gn, |co| { $fut_init(co) })
        };
    }

    #[doc(inline)]
    pub use let_local_gen as let_gen;

    impl GeneratorFlavor for StackLocal {
        type Fut<'a, T: 'a> = dyn Future<Output = T> + 'a;

        type UniquePtr<'a, T: ?Sized + 'a> = &'a mut T;

        type SharedPtr<'a, T: ?Sized + 'a> = &'a T;

        type Cell<T> = Cell<T>;

        fn new_cell<T>(value: T) -> Self::Cell<T> {
            Cell::new(value)
        }

        fn cell_replace<T>(cell: &Self::Cell<T>, other: T) -> T {
            cell.replace(other)
        }
    }

    impl StackFlavor for StackLocal {}

    pub type StackCellSlot<Y, R> = crate::CellSlot<Y, R, StackLocal>;

    pub type StackCo<'a, Y, R> = crate::Co<'a, Y, R, StackLocal>;

    pub type StackGn<'a, Y, R, O> = crate::Gn<'a, Y, R, O, StackLocal>;

    impl<'a, Y, R, O> StackGn<'a, Y, R, O> {
        pub fn new(
            slot: &'a StackCellSlot<Y, R>,
            generator: Pin<&'a mut (dyn Future<Output = O> + 'a)>,
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
    use alloc::rc::Rc;
    use core::cell::Cell;
    use core::future::Future;

    use crate::{CellSlot, GeneratorFlavor, HeapFlavor};

    /// Thread local flavor, for non-`Send + Sync` generators
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub struct HeapLocal;

    impl GeneratorFlavor for HeapLocal {
        type Fut<'a, T: 'a> = dyn Future<Output = T> + 'a;

        type UniquePtr<'a, T: ?Sized + 'a> = Box<T>;

        type SharedPtr<'a, T: ?Sized + 'a> = Rc<T>;

        type Cell<T> = Cell<T>;

        fn new_cell<T>(value: T) -> Self::Cell<T> {
            Cell::new(value)
        }

        fn cell_replace<T>(cell: &Self::Cell<T>, other: T) -> T {
            cell.replace(other)
        }
    }

    impl HeapFlavor for HeapLocal {
        fn new_shared<'a, T: 'a>(value: T) -> Self::SharedPtr<'a, T> {
            Rc::new(value)
        }
    }

    /// Thread local generator controller
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type Co<'a, Y, R> = crate::Co<'a, Y, R, HeapLocal>;

    /// Thread local generator controller holding items with 'static lifetime only
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type StaticCo<Y, R> = Co<'static, Y, R>;

    /// Thread local generator
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type Gn<'a, Y, R, O> = crate::Gn<'a, Y, R, O, HeapLocal>;

    impl<'a, Y, R, O> Gn<'a, Y, R, O> {
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub fn new<Producer, Generator>(producer: Producer) -> Self
        where
            Producer: FnOnce(Co<'a, Y, R>) -> Generator,
            Generator: Future<Output = O> + 'a,
        {
            let co = Co::new_heap(CellSlot::default());
            let slots = Rc::clone(&co.slot);
            let generator = Box::pin(producer(co));
            Self::from_parts(slots, generator)
        }
    }

    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub type StaticGn<Y, R, O> = crate::Gn<'static, Y, R, O, HeapLocal>;
}
