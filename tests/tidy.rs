use std::process::Command;

macro_rules! on_error {
    ($output:expr, $on_error:block) => {{
        if !$output.status.success() {
            println!("{}", String::from_utf8_lossy($output.stdout.as_slice()));
            eprintln!("{}", String::from_utf8_lossy($output.stderr.as_slice()));
            $on_error;
        }
    }};
}

#[test]
fn check_formatting() {
    let output = Command::new("cargo")
        .arg("fmt")
        .arg("--all")
        .arg("--")
        .arg("--check")
        .output()
        .expect("failed to execute command");

    on_error!(output, {
        panic!("Bad formatting, please run 'cargo +stable fmt --all'")
    })
}

#[test]
fn check_doc() {
    let output = Command::new("cargo")
        .arg("doc")
        .env("RUSTDOCFLAGS", "-D warnings")
        .output()
        .expect("failed to execute command");

    on_error!(output, {
        panic!("Fix warnings and errors in documentation")
    })
}

#[test]
fn check_lints() {
    let output = Command::new("cargo")
        .arg("clippy")
        .arg("--workspace")
        .arg("--locked")
        .arg("--tests")
        .arg("--")
        .args(["-D", "warnings"])
        .env("RUSTDOCFLAGS", "-D warnings")
        .output()
        .expect("failed to execute command");

    on_error!(output, { panic!("Fix warnings in code") })
}
