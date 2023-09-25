use genoise::local;
use genoise::{Co, GeneratorFlavor, GnState};

async fn return_borrowed_value<'a, F>(mut co: Co<'_, (), (), F>, value: &'a str) -> &'a str
where
    F: GeneratorFlavor,
{
    co.suspend(()).await;
    value
}

async fn return_borrowed_value_2<'a, F>(mut co: Co<'static, (), (), F>, value: &'a str) -> &'a str
where
    F: GeneratorFlavor,
{
    co.suspend(()).await;
    value
}

fn main() {
    let hello = String::from("hello");

    {
        let mut g = local::Gn::new(|yp| return_borrowed_value(yp, &hello));
        assert!(!g.started());
        assert!(matches!(g.start(), GnState::Suspended(())));
        assert!(g.started());
        assert!(matches!(g.resume(()), GnState::Completed("hello")));
    }

    {
        let mut g = local::Gn::new(|yp| return_borrowed_value_2(yp, &hello));
        assert!(!g.started());
        assert!(matches!(g.start(), GnState::Suspended(())));
        assert!(g.started());
        assert!(matches!(g.resume(()), GnState::Completed("hello")));
    }

    {
        local::let_stacked_gen!(g, co, { return_borrowed_value(co, &hello) });
        assert!(!g.started());
        assert!(matches!(g.start(), GnState::Suspended(())));
        assert!(g.started());
        borrow_stacked_generator(&g);
        assert!(matches!(g.resume(()), GnState::Completed("hello")));
        consume_stacked_generator(g);
    }

    {
        let yield_slot = local::new_stacked_slot();
        let resume_slot = local::new_stacked_slot();
        let co = local::new_stacked_co(&yield_slot, &resume_slot);
        let fut = core::pin::pin!(return_borrowed_value(co, &hello));
        let mut g = local::StackedGn::new(&yield_slot, &resume_slot, fut);
        assert!(!g.started());
        assert!(matches!(g.start(), GnState::Suspended(())));
        assert!(g.started());
        borrow_stacked_generator(&g);
        assert!(matches!(g.resume(()), GnState::Completed("hello")));
        consume_stacked_generator(g);
    }
}

fn borrow_stacked_generator<'gen, 'slot>(g: &local::StackedGn<'gen, 'slot, (), (), &str>) {
    assert!(g.started());
}

fn consume_stacked_generator<'gen, 'slot>(g: local::StackedGn<'gen, 'slot, (), (), &str>) {
    assert!(g.started());
}
