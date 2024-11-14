use std::{fmt::Display, panic::Location, process::exit};

use anyhow::{anyhow, Result};
use log::error;
use miette::Diagnostic;

/// An extension trait for `Option` and `Result` to log errors and then exit.
/// These errors are meant to be seen by the user and are intentional.
pub trait LogExpect<T> {
    fn log_expect(self, msg: &str) -> T;
}

/// An extension trait for converting miette `Diagnostic`s to anyhow `Error`s
pub trait ToAnyhow<T> {
    fn to_anyhow(self) -> Result<T>;
}

impl<T, E: Diagnostic + Send + Sync + 'static> ToAnyhow<T> for Result<T, E> {
    fn to_anyhow(self) -> Result<T> {
        self.map_err(|e| anyhow!("{:?}", miette::Report::new(e)))
    }
}

impl<T, E: Display> LogExpect<T> for Result<T, E> {
    #[track_caller]
    fn log_expect(self, msg: &str) -> T {
        match self {
            Ok(o) => o,
            Err(e) => {
                error!("{msg}: {e}\n    at {}", Location::caller());
                exit(1)
            }
        }
    }
}
