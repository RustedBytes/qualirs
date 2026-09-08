// EXPECT: Excessive Clone
// EXPECT: Arc Mutex Overuse
// EXPECT: Large Future
// EXPECT: Async Trait Overhead
// EXPECT: Interior Mutability Abuse
// EXPECT: Unnecessary Allocation in Loop
// EXPECT: Collect Then Iterate
// EXPECT: Repeated Regex Construction
// EXPECT: Missing Collection Preallocation
// EXPECT: Repeated String Conversion in Hot Path
// EXPECT: Needless Intermediate String Formatting
// EXPECT: Vec Contains in Loop
// EXPECT: Sort Before Min or Max
// EXPECT: Full Sort for Single Element
// EXPECT: Clone Before Move Into Collection
// EXPECT: Inefficient Iterator Step
// EXPECT: Chars Count Length Check
// EXPECT: Repeated Expensive Construction in Loop
// EXPECT: Needless Dynamic Dispatch
// EXPECT: Local Lock in Single-Threaded Scope
// EXPECT: Clone on Copy
// EXPECT: Large Value Passed By Value
// EXPECT: Inline Candidate

#![allow(dead_code, unused_variables)]

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

struct State {
    a: Arc<Mutex<i32>>,
    b: Arc<Mutex<i32>>,
    c: Arc<Mutex<i32>>,
    d: Arc<Mutex<i32>>,
}

struct MutableEverywhere {
    a: RefCell<i32>,
    b: RefCell<i32>,
    c: RefCell<i32>,
    d: RefCell<i32>,
    e: RefCell<i32>,
    f: RefCell<i32>,
}

async fn large_future() {
    let buffer = [0_u8; 4096];
    let _line_000 = 0;
    let _line_001 = 1;
    let _line_002 = 2;
    let _line_003 = 3;
    let _line_004 = 4;
    let _line_005 = 5;
    let _line_006 = 6;
    let _line_007 = 7;
    let _line_008 = 8;
    let _line_009 = 9;
    let _line_010 = 10;
    let _line_011 = 11;
    let _line_012 = 12;
    let _line_013 = 13;
    let _line_014 = 14;
    let _line_015 = 15;
    let _line_016 = 16;
    let _line_017 = 17;
    let _line_018 = 18;
    let _line_019 = 19;
    let _line_020 = 20;
    let _line_021 = 21;
    let _line_022 = 22;
    let _line_023 = 23;
    let _line_024 = 24;
    let _line_025 = 25;
    let _line_026 = 26;
    let _line_027 = 27;
    let _line_028 = 28;
    let _line_029 = 29;
    let _line_030 = 30;
    let _line_031 = 31;
    let _line_032 = 32;
    let _line_033 = 33;
    let _line_034 = 34;
    let _line_035 = 35;
    let _line_036 = 36;
    let _line_037 = 37;
    let _line_038 = 38;
    let _line_039 = 39;
    let _line_040 = 40;
    let _line_041 = 41;
    let _line_042 = 42;
    let _line_043 = 43;
    let _line_044 = 44;
    let _line_045 = 45;
    let _line_046 = 46;
    let _line_047 = 47;
    let _line_048 = 48;
    let _line_049 = 49;
    let _line_050 = 50;
    let _line_051 = 51;
    let _line_052 = 52;
    let _line_053 = 53;
    let _line_054 = 54;
    let _line_055 = 55;
    let _line_056 = 56;
    let _line_057 = 57;
    let _line_058 = 58;
    let _line_059 = 59;
    let _line_060 = 60;
    let _line_061 = 61;
    let _line_062 = 62;
    let _line_063 = 63;
    let _line_064 = 64;
    let _line_065 = 65;
    let _line_066 = 66;
    let _line_067 = 67;
    let _line_068 = 68;
    let _line_069 = 69;
    let _line_070 = 70;
    let _line_071 = 71;
    let _line_072 = 72;
    let _line_073 = 73;
    let _line_074 = 74;
    let _line_075 = 75;
    let _line_076 = 76;
    let _line_077 = 77;
    let _line_078 = 78;
    let _line_079 = 79;
    let _line_080 = 80;
    let _line_081 = 81;
    let _line_082 = 82;
    let _line_083 = 83;
    let _line_084 = 84;
    let _line_085 = 85;
    let _line_086 = 86;
    let _line_087 = 87;
    let _line_088 = 88;
    let _line_089 = 89;
    let _line_090 = 90;
    let _line_091 = 91;
    let _line_092 = 92;
    let _line_093 = 93;
    let _line_094 = 94;
    let _line_095 = 95;
    let _line_096 = 96;
    let _line_097 = 97;
    let _line_098 = 98;
    let _line_099 = 99;
    let _line_100 = 100;
    let _line_101 = 101;
    let _line_102 = 102;
    let _line_103 = 103;
    let _line_104 = 104;
    pending().await;
    let _ = buffer;
}

async fn pending() {}

#[async_trait::async_trait]
trait Repository {
    async fn load(&self);
}

fn clone_many(value: String) {
    let _ = value.clone();
    let _ = value.clone();
    let _ = value.clone();
    let _ = value.clone();
    let _ = value.clone();
    let _ = value.clone();
    let _ = value.clone();
    let _ = value.clone();
    let _ = value.clone();
    let _ = value.clone();
    let _ = value.clone();
    let _ = value.clone();
}

fn allocate_in_loop(items: Vec<i32>) {
    for item in items {
        let text = String::from("constant label");
        consume(&text);
        consume(&item);
    }
}

fn collect_then_iterate(items: Vec<i32>) {
    items
        .iter()
        .map(|item| item + 1)
        .collect::<Vec<_>>()
        .iter()
        .for_each(|item| consume(item));
}

fn regex_in_loop(values: Vec<&str>) {
    for value in values {
        let _ = regex::Regex::new("[a-z]+").unwrap().is_match(value);
    }
}

fn missing_prealloc(items: Vec<i32>) -> Vec<i32> {
    let mut out = Vec::new();
    for item in items {
        out.push(item + 1);
    }
    out
}

fn repeated_to_string(ids: Vec<&str>, prefix: &str) {
    for id in ids {
        lookup(prefix.to_string());
        consume(&id);
    }
}

fn format_temporary(id: u64) {
    let mut line = String::new();
    line.push_str(&format!("id={id}"));
}

fn vec_contains_loop(ids: Vec<i32>, allowed: Vec<i32>) {
    for id in ids {
        if allowed.contains(&id) {
            consume(&id);
        }
    }
}

fn sort_for_min(mut values: Vec<i32>) -> Option<i32> {
    values.sort();
    values.first().copied()
}

fn full_sort_for_first(mut values: Vec<i32>) -> Option<i32> {
    values.sort_unstable();
    values.get(1).copied()
}

fn clone_before_push() {
    let mut items = Vec::new();
    let value = String::from("owned");
    items.push(value.clone());
}

fn inefficient_step(mut values: std::vec::IntoIter<i32>) -> Option<i32> {
    values.skip(3).next()
}

fn chars_count(value: &str) -> bool {
    value.chars().count() == 0
}

struct Client;
impl Client {
    fn new() -> Self {
        Self
    }
}

fn rebuild_client(items: Vec<i32>) {
    for item in items {
        let url = url::Url::parse("https://example.com/items").unwrap();
        consume(&(url, item));
    }
}

trait Job {
    fn run(&self);
}

struct RealJob;
impl Job for RealJob {
    fn run(&self) {}
}

fn dynamic_dispatch(job: Box<dyn Job>) {
    let local_job: Box<dyn Job> = Box::new(RealJob);
    local_job.run();
    job.run();
}

fn local_lock() {
    let value = Mutex::new(0);
    *value.lock().unwrap() += 1;
}

fn clone_copy(count: usize) -> usize {
    let count: usize = count;
    count.clone()
}

fn large_value(bytes: [u8; 64]) {
    consume(&bytes);
}

fn tiny_wrapper(value: &str) -> bool {
    value.is_empty()
}

fn use_tiny_wrapper(value: &str) {
    let _ = tiny_wrapper(value);
    let _ = tiny_wrapper(value);
    let _ = tiny_wrapper(value);
}

fn consume<T>(_value: T) {}
fn lookup(_value: String) {}
