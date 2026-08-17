// aux-build: crossbeam_channel.rs
// aux-build: futures_util.rs
// aux-build: rayon.rs
// aux-build: tokio.rs
// compile-flags: --test

#![warn(unnamed_policy_literals)]
#![allow(dead_code, unused_variables)]

extern crate crossbeam_channel;
extern crate futures_util;
extern crate rayon;
extern crate tokio;

use futures_util::StreamExt;
use std::time::Duration;

struct DeliveryPolicy {
    max_attempts: usize,
    timeout: Duration,
}

struct CustomBuffer;

impl CustomBuffer {
    fn with_capacity(_: usize) -> Self {
        Self
    }
}

fn deliver() -> Result<(), ()> {
    Ok(())
}

const MAX_DELIVERY_ATTEMPTS: usize = 7;

const DELIVERY_TIMEOUT: Duration = Duration::from_secs(45);

fn retries_use_named_constants() -> Result<(), ()> {
    for _attempt in 0..MAX_DELIVERY_ATTEMPTS {
        deliver()?;
    }
    let _timeout = DELIVERY_TIMEOUT;
    Ok(())
}

fn unnamed_retry_bounds() -> Result<(), ()> {
    for _attempt in 0..3 {
        deliver()?;
    }

    let max_delivery_attempts = 4;
    for _attempt in 0..max_delivery_attempts {
        deliver()?;
    }

    let bound = 11;
    for _attempt in 0..bound {
        deliver()?;
    }
    Ok(())
}

fn unnamed_api_policies() {
    let _delay = Duration::from_millis(250);
    let _computed_delay = Duration::from_secs(2 * 60);
    let _negative_delay = Duration::from_secs_f64(-2.5);
    let mut values = Vec::<u8>::with_capacity(64);
    values.reserve_exact(32);
    values.truncate(16);
    let _page = values.iter().skip(5).take(10);
    let _channel = std::sync::mpsc::sync_channel::<u8>(8);
    let _barrier = std::sync::Barrier::new(6);
}

fn unnamed_ecosystem_policies() {
    let _semaphore = tokio::sync::Semaphore::new(20);
    let _channel = tokio::sync::mpsc::channel::<u8>(24);
    let _buffered = vec![1_u8].buffer_unordered(12);
    let _pool = rayon::ThreadPoolBuilder::new().num_threads(6);
    let _bounded = crossbeam_channel::bounded::<u8>(32);
}

fn unnamed_configuration() {
    let _policy = DeliveryPolicy {
        max_attempts: 5,
        timeout: Duration::from_secs(30),
    };
}

fn unnamed_failure_threshold(payload: &[u8]) -> Result<(), ()> {
    if payload.len() > 1024 {
        return Err(());
    }
    Ok(())
}

fn accepted_success_only_result_branch(payload: &[u8]) -> Result<(), ()> {
    if payload.len() > 2048 {
        Ok(())
    } else {
        Ok(())
    }
}

fn accepted_incidental_literals(values: &[u8]) {
    for _index in 0..6 {
        let _sum = 2 + 3;
    }
    if values.len() > 9 {
        let _observed = true;
    }
    let _first = values[3];
    let _identity_delay = Duration::from_secs(1);
    let _custom = CustomBuffer::with_capacity(12);
}

macro_rules! generated_duration {
    () => {
        Duration::from_millis(90)
    };
}

fn accepted_generated_literal() {
    let _generated = generated_duration!();
}

#[test]
fn test_policies_are_checked() {
    let _timeout = Duration::from_millis(15);
}

fn main() {}
