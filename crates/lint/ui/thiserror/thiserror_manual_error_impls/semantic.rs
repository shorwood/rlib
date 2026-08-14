#![allow(dead_code, unknown_lints)]

use std::error::Error;
use std::fmt;

#[derive(Debug)]
struct Marker;

impl fmt::Display for Marker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("marker")
    }
}

impl Error for Marker {}

#[derive(Debug)]
struct LoadError {
    source: std::io::Error,
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("load failed")
    }
}

impl Error for LoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Debug)]
struct ContextError;

impl fmt::Display for ContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("context")
    }
}

impl Error for ContextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[derive(Debug)]
struct GenericError<T>(std::marker::PhantomData<T>);

impl<T: fmt::Debug> fmt::Display for GenericError<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("generic")
    }
}

impl<T: fmt::Debug> Error for GenericError<T> {}

fn main() {}
