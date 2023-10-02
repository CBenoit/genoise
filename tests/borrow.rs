use genoise::{local, GnState};

#[test]
fn check_owned_gn_can_take_arguments_by_ref() {
    async fn generator<'arg, 'resume>(
        mut co: local::Co<'_, &'arg str, &'resume str>,
        arg: &'arg str,
    ) -> &'resume str {
        co.suspend(arg).await
    }

    let arg = String::from("hello");
    let resume_val;

    let mut g = local::Gn::new(|co| generator(co, &arg));

    if let GnState::Suspended(yielded_value) = g.start() {
        assert_eq!(yielded_value.len(), 5);
        resume_val = yielded_value.len().to_string();
        assert!(matches!(g.resume(&resume_val), GnState::Completed("5")));
    } else {
        panic!()
    }
}
