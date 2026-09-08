// EXPECT: Excessive Unwrap
// EXPECT: Unused Result Ignored
// EXPECT: Panic in Library
// EXPECT: Copy + Drop Conflict
// EXPECT: Deref Abuse
// EXPECT: Manual Drop
// EXPECT: Manual Default Constructor
// EXPECT: Manual Option/Result Mapping
// EXPECT: Manual Find/Any Loop
// EXPECT: Needless Explicit Lifetime
// EXPECT: Derivable Impl

#![allow(dead_code, unused_must_use)]

use std::io::Write;
use std::ops::Deref;

pub fn library_panic() {
    panic!("recoverable input error");
}

fn many_unwraps() {
    let _ = Some(1).unwrap();
    let _ = Some(2).unwrap();
    let _ = Some(3).unwrap();
    let _ = Some(4).unwrap();
}

fn ignore_result() {
    let _ = std::fs::write("log.txt", b"ignored");
}

#[derive(Copy, Clone)]
struct Handle(i32);

impl Drop for Handle {
    fn drop(&mut self) {}
}

struct App {
    config: Config,
}

struct Config;

impl Deref for App {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

fn manual_drop(lock: std::sync::MutexGuard<'_, i32>) {
    drop(lock);
}

struct Settings {
    retries: u8,
}

impl Settings {
    fn new() -> Self {
        Self {
            retries: Default::default(),
        }
    }
}

fn manual_map(value: Option<i32>) -> Option<i32> {
    match value {
        Some(item) => Some(item + 1),
        None => None,
    }
}

fn manual_find(values: Vec<i32>) -> bool {
    for value in values {
        if value > 10 {
            return true;
        }
    }
    false
}

fn needless_lifetime<'a>(value: &'a str) -> &'a str {
    value
}

enum Mode {
    Fast,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Fast
    }
}

struct Debuggable {
    value: i32,
}

impl std::fmt::Debug for Debuggable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Debuggable")
    }
}

struct Mechanical {
    value: String,
}
impl Default for Mechanical {
    fn default() -> Self {
        Self {
            value: String::new(),
        }
    }
}
