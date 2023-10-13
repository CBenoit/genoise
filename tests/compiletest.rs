use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

fn check<T: AsRef<Path> + ?Sized>(
    source_file: &T,
    diffing_program: Option<&DiffingProgram>,
    overwrite: bool,
) {
    check_impl(source_file.as_ref(), diffing_program, overwrite);
}

// Minimal `trybuild`-like harness
fn check_impl(source_file: &Path, diffing_program: Option<&DiffingProgram>, overwrite: bool) {
    let output = Command::new("rustc")
        .arg(source_file)
        .args(["--edition", "2021"])
        .args(["--extern", "genoise=./target/debug/libgenoise.rlib"])
        .args(["--out-dir", "./target/debug/"])
        .output()
        .expect("failed to execute command");

    if output.status.success() {
        panic!(
            "Expected test case to fail to compile, but it succeeded.\nSource file: {}",
            source_file.display()
        );
    }

    let mut stderr = String::new();

    for line in String::from_utf8_lossy(&output.stderr).lines() {
        // ignore source file paths
        if line.starts_with("   -->") {
            continue;
        }

        // filter out line numbers to prevent test from failing when modifying source code
        line.chars().fold(true, |filter, c| {
            if filter && c.is_numeric() {
                stderr.push(' ');
                true
            } else {
                stderr.push(c);
                false
            }
        });

        // insert newline
        stderr.push('\n');
    }

    let stderr_file = source_file.with_extension("stderr");
    let actual_stderr_file = stderr_file.with_extension("actual.stderr");

    match std::fs::read_to_string(&stderr_file) {
        Ok(expected_stderr) if expected_stderr != stderr && overwrite => {
            println!("Different from original file, overwriting…");
            std::fs::write(&stderr_file, stderr).expect("fs::write");
        }
        Ok(expected_stderr) if expected_stderr != stderr => {
            std::fs::write(&actual_stderr_file, stderr).expect("fs::write");

            if let Some(diffing_program) = diffing_program {
                diffing_program.diff(&stderr_file, &actual_stderr_file);
            }

            panic!(
                "stderr didn’t match contents of {}; see {}",
                stderr_file.display(),
                actual_stderr_file.display()
            );
        }
        Ok(_) => println!("Ok."),
        Err(_) if overwrite => {
            println!("Writing stderr to {}", stderr_file.display());
            std::fs::write(&stderr_file, stderr).expect("fs::write");
        }
        Err(_) => {
            std::fs::write(&actual_stderr_file, stderr).expect("fs::write");
            panic!(
                "{} doesn’t exist yet; wrote stderr into {}",
                stderr_file.display(),
                actual_stderr_file.display()
            );
        }
    }
}

fn compile_library() {
    let output = Command::new("cargo")
        .arg("build")
        .arg("--lib")
        .output()
        .expect("failed to execute command");

    assert!(output.status.success());
}

struct DiffingProgram {
    program: &'static str,
    args: &'static [&'static str],
    envs: &'static [(&'static str, &'static str)],
}

impl DiffingProgram {
    fn is_available(&self) -> bool {
        Command::new(self.program).arg("--version").output().is_ok()
    }

    fn diff(&self, minus_file: &Path, plus_file: &Path) {
        let output = Command::new(self.program)
            .arg(minus_file)
            .arg(plus_file)
            .args(self.args)
            .envs(self.envs.iter().copied())
            .output()
            .expect("failed to execute command");

        // Using (e)print! so the output is properly captured by the test harness

        if let Ok(stderr) = std::str::from_utf8(&output.stderr) {
            eprint!("{stderr}");
        }

        if let Ok(stdout) = std::str::from_utf8(&output.stdout) {
            print!("{stdout}");
        }
    }
}

fn find_diffing_program() -> Option<&'static DiffingProgram> {
    const CANDIDATES: &[DiffingProgram] = &[
        DiffingProgram {
            program: "delta",
            args: &[],
            envs: &[("DELTA_FEATURES", "-side-by-side")],
        },
        DiffingProgram {
            program: "colordiff",
            args: &[],
            envs: &[],
        },
        DiffingProgram {
            program: "diff",
            args: &["--color=always"],
            envs: &[],
        },
    ];

    CANDIDATES.iter().find(|program| program.is_available())
}

#[test]
#[cfg_attr(miri, ignore)]
fn misuse() {
    compile_library(); // generate .rlib file

    let diffing_program = find_diffing_program();

    let overwrite = std::env::var("GENOISE_COMPILETEST")
        .iter()
        .any(|value| value == "overwrite");

    for entry in std::fs::read_dir("./tests/misuse/").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension() == Some(OsStr::new("rs")) {
            println!("Check {}", path.display());
            check(&path, diffing_program, overwrite);
        }
    }
}
