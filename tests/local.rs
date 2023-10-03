use genoise::local::{Gn, StackGn};

macro_rules! assert_not_send_nor_sync {
    ($type:ty) => {
        const _: fn() = || {
            trait Implemented<T> {
                fn noop() {}
            }

            struct Always;
            impl<T> Implemented<Always> for T {}

            struct IfSendAndSync;
            impl<T: Send + Sync> Implemented<IfSendAndSync> for T {}

            let _ = <$type as Implemented<_>>::noop;
        };
    };
}

#[test]
fn check_heap_gn_is_not_send_nor_sync() {
    assert_not_send_nor_sync!(Gn<'_, (), (), ()>);
}

#[test]
fn check_stacked_gn_is_not_send_nor_sync() {
    assert_not_send_nor_sync!(StackGn<'_, (), (), ()>);
}
