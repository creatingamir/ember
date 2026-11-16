//! The golden-test harness: runs `ember run` on every program in `tests/golden`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use golden::{Case, Outcome};
use libtest_mimic::{Arguments, Failed, Trial};

/// Finds the suite directory at the workspace root.
fn suite_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

/// Runs `ember run` on the case's source and captures what it produced.
fn run_case(case: &Case) -> Result<Outcome, Failed> {
    let output = Command::new(env!("CARGO_BIN_EXE_ember"))
        .arg("run")
        .arg(&case.source)
        .output()
        .map_err(|error| Failed::from(format!("could not start ember: {error}")))?;

    Ok(Outcome {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Runs one case, then blesses its expectation or checks against it.
fn run_trial(case: &Case, bless: bool) -> Result<(), Failed> {
    let outcome = run_case(case)?;
    // golden::Error implements Display, which libtest-mimic turns into a `Failed`.
    if bless {
        golden::bless(case, &outcome)?;
    } else {
        golden::check(case, &outcome)?;
    }
    Ok(())
}

/// Discovers the golden cases and runs them as libtest-mimic trials.
fn main() -> ExitCode {
    let cases = match golden::discover(&suite_dir()) {
        Ok(cases) => cases,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };

    let bless = std::env::var("EMBER_BLESS").is_ok_and(|value| value == "1");

    let mut trials = Vec::new();
    for case in cases {
        let name = case.name.clone();
        // `move` transfers ownership of the case into the closure, so it can run after this loop.
        trials.push(Trial::test(name, move || run_trial(&case, bless)));
    }

    let arguments = Arguments::from_args();
    libtest_mimic::run(&arguments, trials).exit_code()
}
