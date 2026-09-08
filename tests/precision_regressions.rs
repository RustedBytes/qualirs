use qualirs::detectors::{concurrency as c, implementation as i, r#unsafe as u};
use qualirs::{
    analysis::{detector::Detector, engine::Engine},
    domain::{
        config::{Config, PolicyConfig, Precision},
        smell::{FindingConfidence, Severity, Smell},
        source::SourceFile,
    },
};

fn findings(detector: &dyn Detector, code: &str) -> Vec<Smell> {
    detector.detect(&SourceFile::from_source("src/lib.rs".into(), code.into()).unwrap())
}
fn clean(detector: &dyn Detector, code: &str) {
    assert!(
        findings(detector, code).is_empty(),
        "unexpected findings: {:?}",
        findings(detector, code)
    );
}
fn positive(detector: &dyn Detector, code: &str) {
    let found = findings(detector, code);
    assert!(!found.is_empty(), "missing positive: {code}");
    assert!(
        found
            .iter()
            .all(|s| s.confidence == FindingConfidence::High)
    );
}

#[test]
fn safe_mutable_access_is_not_unsafe_aliasing() {
    let d = u::multi_mut_ref_unsafe::MultiMutRefUnsafeDetector;
    clean(
        &d,
        "fn f(mut a: Option<i32>, mut b: Option<i32>) { let _ = a.as_mut(); let _ = b.as_mut(); }",
    );
    clean(
        &d,
        "fn f(a: &mut i32, b: &mut i32) { let x = &mut *a; let y = &mut *b; }",
    );
    clean(
        &d,
        "unsafe fn f(p: *mut i32) { let a = &mut *p; } unsafe fn g(p: *mut i32) { let b = &mut *p; }",
    );
    let result = findings(
        &d,
        "unsafe fn f(p: *mut i32) { let a = &mut *p; let b = &mut *p; use_both(a,b); }",
    );
    assert_eq!(result.len(), 2);
    assert!(
        result
            .iter()
            .all(|s| s.confidence == FindingConfidence::Low)
    );
}

#[test]
fn transmute_requires_exact_resolved_api() {
    let d = u::transmute_usage::TransmuteUsageDetector;
    clean(&d, "fn transmute_label() {} fn f() { transmute_label(); }");
    clean(
        &d,
        "fn transmute(x: i32) -> i32 { x } fn f() { transmute(1); }",
    );
    clean(&d, "mod std {} fn f() { std::mem::transmute(1); }");
    clean(
        &d,
        "use core::mem::transmute as cast; fn f(cast: fn(i32)) { cast(1); }",
    );
    positive(
        &d,
        "use core::mem::{transmute as cast}; unsafe fn f(x:u32) { cast::<u32,f32>(x); }",
    );
    positive(
        &d,
        "mod nested { use std::mem; unsafe fn f(x:u32) { mem::transmute::<u32,f32>(x); } }",
    );
}

#[test]
fn raw_pointer_arithmetic_requires_pointer_type() {
    let d = u::raw_pointer_arithmetic::RawPointerArithmeticDetector;
    clean(&d, "fn f(raw: usize) -> usize { raw.wrapping_add(1) }");
    clean(
        &d,
        "fn f(ptr: *const i32) { let ptr: usize = 1; ptr.wrapping_add(1); }",
    );
    positive(&d, "fn f(p: *const i32) { unsafe { p.add(1); } }");
    positive(
        &d,
        "fn f(x: usize) { unsafe { (x as *const i32).add(1); } }",
    );
}

#[test]
fn aliases_generics_and_shadowed_imports_do_not_invent_types() {
    clean(
        &u::transmute_usage::TransmuteUsageDetector,
        "mod std {} use std::mem::transmute as cast; fn f() { cast(1); }",
    );
    positive(
        &u::transmute_usage::TransmuteUsageDetector,
        "mod std {} use ::std::mem::transmute as cast; unsafe fn f(x:u32) { cast::<u32,f32>(x); }",
    );
    clean(
        &i::unused_result::UnusedResultDetector,
        "fn value<Result>() -> Result { todo!() } fn f() { let _=value::<u32>(); }",
    );
    clean(
        &i::unused_result::UnusedResultDetector,
        "fn value()->Result<(),Error> { todo!() } fn f() { use custom::value; let _=value(); }",
    );
    clean(
        &u::multi_mut_ref_unsafe::MultiMutRefUnsafeDetector,
        "unsafe fn f(p:*mut i32,q:*mut i32) { let a=&mut *p; let p=q; let b=&mut *p; }",
    );
    clean(
        &c::holding_lock_across_await::HoldingLockAcrossAwaitDetector,
        "async fn f(m:&std::sync::Mutex<i32>) { let g=m.lock().unwrap(); (|| drop(g))(); work().await; }",
    );
    clean(
        &c::holding_lock_across_await::HoldingLockAcrossAwaitDetector,
        "async fn f(m:&std::sync::Mutex<i32>) { let g=m.lock().unwrap(); return; work().await; }",
    );
}

#[test]
fn async_lock_types_and_execution_boundaries() {
    let d = c::std_mutex_in_async::StdMutexInAsyncDetector;
    clean(&d, "async fn f(m: tokio::sync::Mutex<i32>) {}");
    clean(
        &d,
        "use tokio::sync::Mutex as M; async fn f(m: M<i32>) { let g=m.lock().await; }",
    );
    let result = findings(
        &d,
        "use std::sync::Mutex as M; async fn f(m: M<i32>) { let g=m.lock(); }",
    );
    assert!(!result.is_empty());
    assert!(
        result
            .iter()
            .all(|s| s.confidence == FindingConfidence::Low)
    );
    let d = c::blocking_in_async::BlockingInAsyncDetector;
    clean(
        &d,
        "async fn f() { tokio::task::spawn_blocking(|| std::fs::read(\"x\")).await; }",
    );
    clean(
        &d,
        "async fn f() { let worker = || std::thread::sleep(duration()); }",
    );
    clean(&d, "async fn f() { fn worker() { std::fs::read(\"x\"); } }");
    clean(
        &d,
        "async fn f() { let handle = std::thread::spawn(|| {}); let out = std::io::stdout(); }",
    );
    positive(
        &d,
        "use std::fs::read as read_file; async fn f() { read_file(\"x\"); }",
    );
    positive(
        &d,
        "struct S; impl S { async fn f(&self) { std::fs::read(\"x\"); } }",
    );
    positive(&d, "fn f() { let task = async { std::fs::read(\"x\"); }; }");
}

#[test]
fn guard_must_be_live_at_executed_await() {
    let d = c::holding_lock_across_await::HoldingLockAcrossAwaitDetector;
    clean(
        &d,
        "async fn f(m: &std::sync::Mutex<i32>) { let g=m.lock().unwrap(); let fut=async { work().await }; drop(g); fut.await; }",
    );
    clean(
        &d,
        "async fn f(m: &std::sync::Mutex<i32>) { let g=m.lock().unwrap(); { std::mem::drop(g); } work().await; }",
    );
    clean(
        &d,
        "async fn f(m: &std::sync::Mutex<i32>) { { let g=m.lock().unwrap(); } work().await; }",
    );
    clean(
        &d,
        "async fn f(m: &std::sync::Mutex<i32>) { let g=m.lock().unwrap(); { drop(g); work() }.await; }",
    );
    clean(
        &d,
        "async fn f(reader: Reader) { let count=reader.read(); work().await; }",
    );
    clean(
        &d,
        "async fn f(m: tokio::sync::Mutex<i32>) { let g=m.lock().await; work().await; }",
    );
    positive(
        &d,
        "use std::sync::Mutex; async fn f(m:&Mutex<i32>) { let g=m.lock().unwrap(); work().await; drop(g); }",
    );
}

#[test]
fn channel_receive_requires_synchronous_type() {
    let d = c::blocking_channel_in_async::BlockingChannelInAsyncDetector;
    clean(
        &d,
        "async fn f(mut rx: tokio::sync::mpsc::Receiver<i32>) { let pending=rx.recv(); pending.await; }",
    );
    clean(&d, "async fn f(custom: Custom) { custom.recv(); }");
    positive(
        &d,
        "use std::sync::mpsc::Receiver as R; async fn f(rx:R<i32>) { rx.recv(); }",
    );
}

#[test]
fn handled_and_non_result_values_are_not_discarded_errors() {
    let d = i::unused_result::UnusedResultDetector;
    clean(&d, "fn f() { let _ = std::fs::read(\"x\").unwrap(); }");
    clean(
        &d,
        "fn f() -> std::io::Result<()> { let _ = std::fs::read(\"x\")?; Ok(()) }",
    );
    clean(
        &d,
        "async fn tick() {} async fn f() { let _ = tick().await; }",
    );
    clean(
        &d,
        "fn try_number() -> u32 { 1 } fn f() { let _ = try_number(); }",
    );
    positive(&d, "fn f() { let _ = std::fs::read(\"x\"); }");
    positive(
        &d,
        "async fn result() -> Result<(), Error> { todo!() } async fn f() { let _=result().await; }",
    );
    positive(
        &d,
        "fn nested() -> Result<Result<(), Error>, Error> { todo!() } fn f() { let _=nested().unwrap(); }",
    );
}

#[test]
fn copy_evidence_obeys_shadowing_and_scopes() {
    let d = i::clone_on_copy::CloneOnCopyDetector;
    clean(
        &d,
        "fn a() { let value:u32=0; } fn b(value:String) { let _=value.clone(); }",
    );
    clean(
        &d,
        "fn a() { let value:u32=0; let value=String::new(); let _=value.clone(); }",
    );
    clean(
        &d,
        "fn a(value:u32) { let f=|value:String| value.clone(); }",
    );
    clean(
        &d,
        "fn a(value:u32) { match v { Some(value) => value.clone(), None => todo!() }; }",
    );
    clean(&d, "fn a(value:u32, obj:Obj) { obj.value.clone(); }");
    positive(&d, "fn f(value:u32) { value.clone(); }");
    positive(&d, "type Count=u32; fn f(value:Count) { value.clone(); }");
    clean(
        &d,
        "type Count=u32; fn f(value:Count) { struct Count; fn inner(value:Count) { value.clone(); } }",
    );
    positive(
        &d,
        "fn f() { let value:u32=0; { let value=String::new(); value.clone(); } value.clone(); }",
    );
}

#[test]
fn excluded_test_items_do_not_inflate_module_metrics() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    let mut code = String::from("fn production() {}\n");
    for i in 0..40 {
        code.push_str(&format!("#[cfg(test)] fn fixture_{i}() {{}}\n"));
    }
    std::fs::write(&file, &code).unwrap();
    for (skip, count) in [(false, 2), (true, 0)] {
        let mut config = Config {
            precision: Precision::Exploratory,
            policy: PolicyConfig {
                skip_tests: skip,
                ..Default::default()
            },
            ..Default::default()
        };
        config.thresholds.arch.god_module_loc = 10;
        config.thresholds.arch.god_module_items = 10;
        let mut engine = Engine::new(config);
        engine.register(Box::new(
            qualirs::detectors::architecture::god_module::GodModuleDetector,
        ));
        let report = engine.analyze(&file);
        assert_eq!(report.smells.len(), count);
        assert!(
            report
                .smells
                .iter()
                .all(|s| s.location.line_end == code.lines().count())
        );
    }
}

#[test]
fn filtering_preserves_unicode_columns_and_handles_test_statements() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    let code = "#[cfg(test)] fn café() { let _ = \"😀\"; } fn prod(s:&str) { s.chars().count()==0; }\nfn other(s:&str) { #[cfg(test)] let n = s.chars().count()==0; }\n";
    std::fs::write(&file, code).unwrap();
    let mut engine = Engine::new(Config::default());
    engine.register(Box::new(
        i::chars_count_length_check::CharsCountLengthCheckDetector,
    ));
    let report = engine.analyze(&file);
    assert_eq!(report.smells.len(), 1);
    let expected = code[..code.find("s.chars()").unwrap()].chars().count();
    assert_eq!(report.smells[0].location.column, Some(expected));
    assert_eq!(report.smells[0].location.line_start, 1);
}

#[test]
fn cli_safe_cases_exit_successfully_and_precision_is_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    std::fs::write(
        &file,
        r#"
fn safe(mut a: Option<i32>, mut b: Option<i32>) { let _ = a.as_mut(); let _ = b.as_mut(); }
fn transmute_label() {}
fn caller() { transmute_label(); }
fn unicode(s: &str) -> bool { s.chars().count() == 2 }
fn handled() -> std::io::Result<()> { let _ = std::fs::read("x")?; Ok(()) }
"#,
    )
    .unwrap();
    for precision in ["conservative", "balanced", "exploratory"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_qualirs"))
            .args(["--precision", precision, "--format", "json"])
            .arg(&file)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let smells = report["smells"].as_array().unwrap();
        assert!(
            !smells
                .iter()
                .any(|s| matches!(s["code"].as_str(), Some("Q0090" | "Q0088" | "Q0068")))
        );
        assert_eq!(
            smells.iter().any(|s| s["code"] == "Q0060"),
            precision == "exploratory"
        );
    }
}

#[test]
fn collection_semantics_and_later_uses_are_preserved() {
    let d = i::collect_then_iterate::CollectThenIterateDetector;
    clean(
        &d,
        "fn f(v:Vec<u32>)->usize { v.into_iter().collect::<std::collections::HashSet<_>>().len() }",
    );
    clean(
        &d,
        "fn f(v:Vec<(u32,u32)>)->usize { v.into_iter().collect::<std::collections::HashMap<_,_>>().len() }",
    );
    positive(
        &d,
        "fn f(v:Vec<u32>)->usize { v.into_iter().collect::<Vec<_>>().len() }",
    );
    let d = i::full_sort_for_single_element::FullSortForSingleElementDetector;
    clean(
        &d,
        "fn f(mut v:Vec<i32>) { v.sort(); let x=v[1]; consume(x,v); }",
    );
    clean(&d, "fn f(v:&mut Vec<i32>) { v.sort(); let x=v[1]; }");
    clean(
        &d,
        "fn f(mut v:Vec<i32>, cond:bool) { if cond { v.sort(); let x=v[1]; } consume(v); }",
    );
    positive(&d, "fn f(mut v:Vec<i32>)->i32 { v.sort_unstable(); v[1] }");
    let d = i::sort_before_min_max::SortBeforeMinMaxDetector;
    clean(
        &d,
        "fn f(mut v:Vec<i32>) { v.sort(); let x=v.first(); consume(v); }",
    );
    positive(
        &d,
        "fn f(mut v:Vec<i32>)->Option<i32> { v.sort(); v.first().copied() }",
    );
    let d = i::clone_before_move_into_collection::CloneBeforeMoveIntoCollectionDetector;
    clean(
        &d,
        "fn f(value:String,out:&mut Vec<String>) { let borrowed=&value; out.push(value.clone()); consume(borrowed); }",
    );
    clean(
        &d,
        "fn f(value:String,out:&mut Vec<String>) { loop { out.push(value.clone()); } }",
    );
    clean(
        &d,
        "fn f(value:String,out:&mut Vec<String>, yes:bool) { if yes { out.push(value.clone()); } consume(value); }",
    );
    positive(
        &d,
        "fn f(value:String,out:&mut Vec<String>) { out.push(value.clone()); }",
    );
    let d = i::vec_contains_in_loop::VecContainsInLoopDetector;
    clean(
        &d,
        "fn f(v:Vec<i32>) { let v:std::collections::HashSet<i32>=todo!(); for i in 0..10 { v.contains(&i); } }",
    );
    positive(
        &d,
        "fn f(v:Vec<i32>) { for i in 0..10 { v.contains(&i); } }",
    );
}

#[test]
fn unicode_counting_and_arithmetic_are_preserved() {
    let d = i::chars_count_length_check::CharsCountLengthCheckDetector;
    clean(&d, "fn f(s:&str)->usize { s.chars().count()+1 }");
    clean(&d, "fn f(s:&str)->usize { s.chars().count()*2 }");
    let found = findings(&d, "fn f(s:&str)->bool { s.chars().count()==2 }");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].confidence, FindingConfidence::Low);
    positive(&d, "fn f(s:&str)->bool { s.chars().count()==0 }");
    positive(&d, "fn f(s:&str)->bool { 0<s.chars().count() }");
}

#[test]
fn custom_derives_and_lifetime_bounds_are_preserved() {
    let d = i::derivable_impl::DerivableImplDetector;
    clean(
        &d,
        "struct Secret(String); impl std::fmt::Debug for Secret { fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result { f.write_str(\"REDACTED\") } }",
    );
    clean(
        &d,
        "struct S { x:u32 } impl Clone for S { fn clone(&self)->Self { Self {x:self.x+1} } }",
    );
    clean(
        &d,
        "struct S<T> { x:T } impl<T> Default for S<T> { fn default()->Self { Self {x:Default::default()} } }",
    );
    positive(
        &d,
        "struct S { x:String } impl Default for S { fn default()->Self { Self {x:String::new()} } }",
    );
    positive(
        &d,
        "struct S { x:String } impl Clone for S { fn clone(&self)->Self { Self {x:self.x.clone()} } }",
    );
    let d = i::needless_explicit_lifetime::NeedlessExplicitLifetimeDetector;
    clean(&d, "fn f<'a:'static>(x:&'a str)->&'static str { x }");
    clean(&d, "fn f<'a>(x:&'a str,y:Holder<'a>)->&'a str { x }");
    clean(&d, "fn f<'a>(x:&'a str)->&'a str { let y:&'a str=x; y }");
    positive(&d, "fn f<'a>(x:&'a str)->&'a str { x }");
}

#[test]
fn match_literal_contents_and_guards_are_semantic() {
    let d = i::duplicate_match_arms::DuplicateMatchArmsDetector;
    clean(
        &d,
        "fn f(x:bool)->&'static str { match x { true=>\"a b\", false=>\"ab\" } }",
    );
    clean(
        &d,
        "fn f(x:u32)->u32 { match x { 1 if check()=>10, _=>10 } }",
    );
    positive_medium(
        &d,
        "fn f(x:u32)->u32 { match x { 1=>work(), 2=>work(), _=>0 } }",
    );
}
fn positive_medium(d: &dyn Detector, code: &str) {
    assert!(!findings(d, code).is_empty());
}

#[test]
fn returned_handles_and_non_handle_spawns_are_preserved() {
    let d = c::spawn_without_join::SpawnWithoutJoinDetector;
    clean(
        &d,
        "fn f()->std::thread::JoinHandle<()> { std::thread::spawn(||{}) }",
    );
    clean(
        &d,
        "fn f()->std::thread::JoinHandle<()> { return std::thread::spawn(||{}); }",
    );
    clean(&d, "fn f() { let h=std::thread::spawn(||{}); h.join(); }");
    positive(
        &d,
        "use std::thread::spawn as start; fn f() { start(||{}); }",
    );
    let d = c::dropped_join_handle::DroppedJoinHandleDetector;
    clean(&d, "fn f() { let _=rayon::spawn(||{}); }");
    clean(&d, "fn spawn() {} fn f() { let _=spawn(); }");
    positive(&d, "fn f() { let _=tokio::spawn(async {}); }");
}

#[test]
fn inline_test_policy_precision_locations_and_ignores() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    std::fs::write(&file, "#[cfg(test)]\nmod tests { fn f(x:u32) { unsafe { std::mem::transmute::<u32,f32>(x); } } }\n#[cfg(not(test))]\nfn prod(x:u32) { unsafe { std::mem::transmute::<u32,f32>(x); } }\n#[cfg(any(test, feature=\"x\"))]\nfn shared(x:u32) { unsafe { std::mem::transmute::<u32,f32>(x); } }\n#[cfg(all(test, feature=\"x\"))]\nfn only_test(x:u32) { unsafe { std::mem::transmute::<u32,f32>(x); } }\n").unwrap();
    for skip in [false, true] {
        let mut engine = Engine::new(Config {
            policy: PolicyConfig {
                skip_tests: skip,
                ..Default::default()
            },
            ..Default::default()
        });
        engine.register(Box::new(u::transmute_usage::TransmuteUsageDetector));
        let report = engine.analyze(&file);
        assert!(report.parse_errors.is_empty());
        assert_eq!(report.smells.len(), if skip { 2 } else { 4 });
        if skip {
            assert_eq!(
                report
                    .smells
                    .iter()
                    .map(|s| s.location.line_start)
                    .collect::<Vec<_>>(),
                vec![4, 6]
            );
        }
    }
    clean(
        &u::transmute_usage::TransmuteUsageDetector,
        "#[cfg(test)] mod tests { unsafe fn f(x:u32) { std::mem::transmute::<u32,f32>(x); } }",
    );
    std::fs::write(&file,"fn f(s:&str) { s.chars().count()==2; }\n// qualirs:ignore Q0088\nunsafe fn ignored(x:u32) { std::mem::transmute::<u32,f32>(x); }\n").unwrap();
    for (precision, count) in [
        (Precision::Conservative, 0),
        (Precision::Balanced, 0),
        (Precision::Exploratory, 1),
    ] {
        let mut engine = Engine::new(Config {
            precision,
            ..Default::default()
        });
        engine.register(Box::new(
            i::chars_count_length_check::CharsCountLengthCheckDetector,
        ));
        engine.register(Box::new(u::transmute_usage::TransmuteUsageDetector));
        let report = engine.analyze(&file);
        assert_eq!(report.smells.len(), count);
        assert!(
            report
                .smells
                .iter()
                .all(|s| s.severity != Severity::Critical)
        );
    }
}
