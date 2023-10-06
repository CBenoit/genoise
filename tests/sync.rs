use genoise::sync::{Gn, StackGn};

macro_rules! assert_send_and_sync {
    ($type:ty) => {
        const _: fn() = || {
            fn assert_impl<T: Send>() {}
            assert_impl::<$type>();
        };
    };
}

#[test]
fn check_heap_gn_is_send_and_sync() {
    assert_send_and_sync!(Gn<'_, (), (), ()>);
}

#[test]
fn check_stack_gn_is_send_and_sync() {
    assert_send_and_sync!(StackGn<'_, (), (), ()>);
}
