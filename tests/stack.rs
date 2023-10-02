use genoise::{local, sync, Co, GeneratorFlavor, GnState};

async fn generator<F: GeneratorFlavor>(_: Co<'_, (), (), F>) {}

#[test]
fn check_yield_type_is_inferred_local() {
    let _ = || {
        local::let_gen!(g, generator);
        g.started();
    };
}

#[test]
fn check_resume_type_is_inferred_local() {
    let _ = || {
        local::let_gen!(g, generator);
        matches!(g.start(), GnState::Suspended(()));
    };
}

#[test]
fn check_return_type_is_inferred_local() {
    let _ = || {
        local::let_gen!(g, generator);
        matches!(g.start(), GnState::Suspended(()));
        g.resume(());
    };
}

#[test]
fn check_yield_type_is_inferred_sync() {
    let _ = || {
        sync::let_gen!(g, generator);
        g.started();
    };
}

#[test]
fn check_resume_type_is_inferred_sync() {
    let _ = || {
        sync::let_gen!(g, generator);
        matches!(g.start(), GnState::Suspended(()));
    };
}

#[test]
fn check_return_type_is_inferred_sync() {
    let _ = || {
        sync::let_gen!(g, generator);
        matches!(g.start(), GnState::Suspended(()));
        g.resume(());
    };
}
