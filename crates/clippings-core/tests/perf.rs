//! Performance checks for spec section 13. Run with
//! `cargo test --release -p clippings-core --test perf -- --ignored --nocapture`.

use clippings_core::decorations::decorate;
use clippings_core::index::{EffectiveFile, Source, SourcedTodo};
use clippings_core::pattern;
use clippings_core::scanner::scan_text;
use clippings_core::settings::Settings;
use clippings_core::view::delta::delta;
use clippings_core::view::render::View;
use std::path::PathBuf;
use std::time::Instant;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

#[test]
#[ignore]
fn view_rebuild_and_diff_with_10k_todos_under_15ms() {
    let s = Settings::default();
    let p = pattern::build(&s.core()).unwrap();
    // 500 files x 20 todos in 50 folders.
    let text: String = (0..20)
        .map(|i| format!("code();\n// TODO item {i}\n"))
        .collect();
    let todos = scan_text(&p, text.as_bytes(), "a.ts");
    let paths: Vec<PathBuf> = (0..500)
        .map(|i| PathBuf::from(format!("/w/d{}/f{i}.ts", i % 50)))
        .collect();
    let files: Vec<EffectiveFile> = paths
        .iter()
        .map(|path| EffectiveFile {
            path: Some(path.as_path()),
            uri: None,
            source: Source::Disk,
            todos: todos
                .iter()
                .map(|todo| SourcedTodo {
                    buffer_uri: None,
                    todo,
                })
                .collect(),
        })
        .collect();
    let roots = vec![PathBuf::from("/w")];
    let mut old = View::build(&s, &files, &roots);
    let mut times = Vec::new();
    for _ in 0..15 {
        let start = Instant::now();
        let new = View::build(&s, &files, &roots);
        let _ = delta(&old, &new, false);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        old = new;
    }
    let m = median(times);
    println!("view rebuild + diff, 10,000 todos: {m:.2} ms");
    assert!(m < 15.0, "{m:.2} ms");
}

#[test]
#[ignore]
fn decorations_for_a_50k_line_buffer_under_20ms() {
    let s = Settings::default();
    let p = pattern::build(&s.core()).unwrap();
    let text: String = (0..50_000)
        .map(|i| {
            if i % 100 == 0 {
                format!("// TODO {i}\n")
            } else {
                format!("let x{i} = {i};\n")
            }
        })
        .collect();
    let mut times = Vec::new();
    for _ in 0..15 {
        let start = Instant::now();
        let todos = scan_text(&p, text.as_bytes(), "a.ts");
        let d = decorate(text.as_bytes(), &todos, &s, &p);
        assert_eq!(d["TODO"].len(), 500);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let m = median(times);
    println!("scan + decorations, 50,000 lines: {m:.2} ms");
    assert!(m < 20.0, "{m:.2} ms");
}
