//! Per-component memory breakdown of what `clippings lsp` holds, built from a
//! real repository through the public API (docs/benchmarks/2026-09-memory.md).
//!
//! A counting `#[global_allocator]` wraps `System` and records the bytes live
//! after each step and the peak within it. It counts both the bytes each
//! allocation asked for and, on macOS, the bytes malloc actually reserved
//! for it (`malloc_size`), which adds size-class rounding. Neither includes
//! pages the allocator keeps after a free, thread stacks or the binary; the
//! process measurements in `scripts/memprofile.py` cover those.
//!
//! Run:
//!
//! ```text
//! cargo run --release -p clippings-core --example memprofile -- ~/tilli/tilliX \
//!     [--hidden] [--no-ignore] [--open 20] [--big FILE] [--tags-file tags.json]
//!     [--dump-seen LIST]
//! cargo run --release -p clippings-core --example memprofile -- --fake FILES TODOS ROOT
//! ```
//!
//! Prints a table, then one `JSON {...}` line with every number.

use clippings_core::admission::Admission;
use clippings_core::decorations::decorate;
use clippings_core::documents::{Document, Documents};
use clippings_core::fs::{Fs, NativeFs};
use clippings_core::index::{BufferEntry, EffectiveContext, Index};
use clippings_core::model::Todo;
use clippings_core::pattern;
use clippings_core::scanner::scan_text;
use clippings_core::settings::Settings;
use clippings_core::styles::IconDescriptor;
use clippings_core::view::delta::delta;
use clippings_core::view::render::{NodeCommand, View, ViewNode};
use clippings_core::view::Node;
use clippings_core::walker::walk_and_scan;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[path = "support/counting.rs"]
mod counting;
use counting::{peak_above, reset_peak, snap, Snap};

fn delta_of(before: Snap, after: Snap) -> Value {
    json!({
        "bytes": after.live as i64 - before.live as i64,
        "reserved": after.real as i64 - before.real as i64,
        "allocs": after.allocs as i64 - before.allocs as i64,
    })
}

/// Size of `x` measured as the bytes freed by dropping it.
fn freed_by<T>(x: T) -> Value {
    let before = snap();
    drop(x);
    delta_of(snap(), before)
}

fn mib(b: i64) -> String {
    format!("{:.2} MiB", b as f64 / 1_048_576.0)
}

struct Args {
    root: PathBuf,
    hidden: bool,
    no_ignore: bool,
    open: usize,
    big: Option<PathBuf>,
    tags_file: Option<PathBuf>,
    dump_seen: Option<PathBuf>,
}

fn args() -> Args {
    let mut a = Args {
        root: PathBuf::new(),
        hidden: false,
        no_ignore: false,
        open: 20,
        big: None,
        tags_file: None,
        dump_seen: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(s) = it.next() {
        match s.as_str() {
            "--hidden" => a.hidden = true,
            "--no-ignore" => a.no_ignore = true,
            "--open" => a.open = it.next().and_then(|n| n.parse().ok()).expect("--open N"),
            "--big" => a.big = it.next().map(PathBuf::from),
            "--tags-file" => a.tags_file = it.next().map(PathBuf::from),
            "--dump-seen" => a.dump_seen = it.next().map(PathBuf::from),
            _ => a.root = PathBuf::from(s),
        }
    }
    assert!(
        !a.root.as_os_str().is_empty(),
        "usage: memprofile ROOT [--hidden] [--no-ignore] [--open N] [--big FILE] [--tags-file F] [--dump-seen LIST]"
    );
    a.root = std::fs::canonicalize(&a.root).expect("root");
    a
}

/// Owned heap bytes of one todo's fields, excluding the `Todo` itself.
fn todo_heap(t: &Todo) -> usize {
    t.tag.capacity()
        + t.before.capacity()
        + t.after.capacity()
        + t.sub_tag.as_ref().map_or(0, String::capacity)
        + t.extra_lines.capacity() * std::mem::size_of::<clippings_core::model::ExtraLine>()
        + t.extra_lines
            .iter()
            .map(|e| e.text.capacity())
            .sum::<usize>()
}

/// `--fake FILES TODOS ROOT`: builds the index and view through the real
/// API from fabricated paths under ROOT (no disk access), laid out like
/// `scripts/memprofile.py generate`, so the view's cost can be measured at
/// any root path length and todo count.
fn fake(files: usize, per: usize, root: &str) {
    let settings: Settings = serde_json::from_value(json!({
        "general": { "tags": ["BUG", "HACK", "FIXME", "TODO", "XXX", "[ ]", "QUESTION"] },
    }))
    .unwrap();
    let pat = pattern::build(&settings.core()).unwrap();
    let text: String = (0..per)
        .map(|t| format!("  // TODO fix the edge case number {t} in handler 1234\n  code();\n"))
        .collect();
    let template = scan_text(&pat, text.as_bytes(), "a.ts");
    let roots = vec![PathBuf::from(root)];
    let s0 = snap();
    let mut index = Index::new();
    for i in 0..files {
        let path = PathBuf::from(format!(
            "{root}/pkg{:02}/src/mod{:02}/sub{}/file{i}.ts",
            i % 40,
            (i / 40) % 25,
            (i / 1000) % 10
        ));
        index.set_disk(path, template.clone());
    }
    let s1 = snap();
    let ctx = EffectiveContext {
        mode: settings.tree.scan_mode,
        walked_roots: &roots,
        active_uri: None,
    };
    let effective = index.effective(&ctx);
    let s2 = reset_peak();
    let view = View::build(&settings, &effective, &roots);
    let (build_peak, _) = peak_above(s2);
    let s3 = snap();
    let ids: usize = view.arena.nodes.iter().map(|n| n.id.len()).sum();
    let nodes = view.arena.nodes.len();
    let todos = files * template.len();
    let index_b = s1.live - s0.live;
    let view_b = s3.live - s2.live;
    println!(
        "JSON {}",
        json!({
            "root": root, "root_len": root.len(), "files": files, "todos": todos,
            "index_bytes": index_b, "view_bytes": view_b, "view_build_peak": build_peak,
            "arena_nodes": nodes, "avg_id_len": ids as f64 / nodes.max(1) as f64,
            "index_per_todo": index_b as f64 / todos.max(1) as f64,
            "view_per_todo": view_b as f64 / todos.max(1) as f64,
            "breakdown": view_breakdown(&view),
        })
    );
}

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    if raw.get(1).map(String::as_str) == Some("--fake") {
        let n = |i: usize| raw[i].parse::<usize>().expect("--fake FILES TODOS ROOT");
        fake(n(2), n(3), &raw[4]);
        return;
    }
    let a = args();
    let base = snap();
    let mut out = serde_json::Map::new();
    let mut rows: Vec<(String, i64, String)> = Vec::new();
    let row = |rows: &mut Vec<(String, i64, String)>, name: &str, v: &Value, note: String| {
        rows.push((name.to_string(), v["bytes"].as_i64().unwrap_or(0), note));
    };

    // --- Settings and pattern, as the server builds them at initialize.
    let s0 = snap();
    let tags: Value = match &a.tags_file {
        Some(f) => serde_json::from_slice(&std::fs::read(f).expect("tags file")).unwrap(),
        None => json!(["BUG", "HACK", "FIXME", "TODO", "XXX", "[ ]", "QUESTION"]),
    };
    let settings: Settings = serde_json::from_value(json!({
        "general": { "tags": tags, "statusBar": "total" },
        "filtering": { "includeHiddenFiles": a.hidden },
    }))
    .expect("settings");
    let mut core = settings.core();
    core.respect_ignore_files = !a.no_ignore;
    let pat = Arc::new(pattern::build(&core).expect("pattern"));
    let fs: Arc<dyn Fs> = Arc::new(NativeFs);
    let roots = vec![a.root.clone()];
    let admission = Admission::new(&core, roots.clone(), fs.clone()).expect("admission");
    let s1 = snap();
    let v = delta_of(s0, s1);
    row(
        &mut rows,
        "settings + pattern + admission (fresh)",
        &v,
        String::new(),
    );
    out.insert("settings_pattern_admission".into(), v);

    // --- First walk: peak during the scan, then what it hands back.
    let before_walk = reset_peak();
    let t = std::time::Instant::now();
    let outcome =
        walk_and_scan(&core, &roots, &pat, fs.clone(), &AtomicBool::new(false)).expect("walk");
    let walk_ms = t.elapsed().as_secs_f64() * 1000.0;
    let (walk_peak, walk_peak_real) = peak_above(before_walk);
    let after_walk = snap();
    let seen_count = outcome.seen.len();
    let files_with_todos = outcome.files.len();
    let todo_count: usize = outcome.files.iter().map(|f| f.todos.len()).sum();
    if let Some(f) = &a.dump_seen {
        let list: Vec<String> = outcome
            .seen
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        std::fs::write(f, list.join("\n") + "\n").expect("dump seen");
    }
    let seen_path_bytes: usize = outcome.seen.iter().map(|p| p.as_os_str().len()).sum();
    out.insert(
        "walk".into(),
        json!({
            "ms": walk_ms,
            "seen_files": seen_count,
            "files_with_todos": files_with_todos,
            "todos": todo_count,
            "peak_bytes": walk_peak,
            "peak_reserved": walk_peak_real,
            "outcome": delta_of(before_walk, after_walk),
            "seen_path_bytes": seen_path_bytes,
        }),
    );
    rows.push((
        "walk peak above pre-walk (transient)".into(),
        walk_peak as i64,
        format!("{seen_count} files admitted, {walk_ms:.0} ms"),
    ));

    // --- Index: what the scheduler keeps. `seen` is dropped after apply.
    let s2 = snap();
    let mut index = Index::new();
    let before_apply = reset_peak();
    let WalkParts { files, seen } = split(outcome);
    index.apply_walk(&roots, files, &seen, true);
    let (apply_peak, _) = peak_above(before_apply);
    drop(seen);
    out.insert("index_after_apply".into(), delta_of(s2, snap()));

    // Theoretical content of the index, from capacities.
    let todo_size = std::mem::size_of::<Todo>();
    let mut index_paths = 0usize;
    let mut index_vec = 0usize;
    let mut index_strings = 0usize;
    let mut disk_files = 0usize;
    for p in walked_paths(&index, &roots) {
        let todos = index.disk(&p).unwrap_or(&[]);
        disk_files += 1;
        index_paths += p.as_os_str().len();
        index_vec += std::mem::size_of_val(todos);
        index_strings += todos.iter().map(todo_heap).sum::<usize>();
    }
    out.insert(
        "index_content".into(),
        json!({
            "files": disk_files,
            "todo_size": todo_size,
            "path_bytes": index_paths,
            "todo_structs": index_vec,
            "todo_strings": index_strings,
            "apply_peak_bytes": apply_peak,
        }),
    );

    // --- Admission after events have touched every directory: the ignore
    // cache holds one entry per directory it has been asked about.
    let s4 = snap();
    let mut admitted = 0usize;
    for p in walked_paths(&index, &roots) {
        admitted += admission.admits_disk(&p) as usize;
    }
    let s5 = snap();
    let v = delta_of(s4, s5);
    row(
        &mut rows,
        "ignore cache, warmed by every indexed file",
        &v,
        format!("{admitted} of {disk_files} admitted"),
    );
    out.insert("ignore_cache_warm".into(), v);

    // --- View: effective files (transient) and the built view.
    let walked = roots.clone();
    let ctx = EffectiveContext {
        mode: settings.tree.scan_mode,
        walked_roots: &walked,
        active_uri: None,
    };
    let s6 = reset_peak();
    let effective = index.effective(&ctx);
    let s7 = snap();
    let view = View::build(&settings, &effective, &roots);
    let (view_build_peak, _) = peak_above(s6);
    let s8 = snap();
    let arena_nodes = view.arena.nodes.len();
    let id_bytes: usize = view.arena.nodes.iter().map(|n| n.id.len()).sum();
    let max_id = view
        .arena
        .nodes
        .iter()
        .map(|n| n.id.len())
        .max()
        .unwrap_or(0);
    let rendered = view.nodes.len();
    let view_parts = view_breakdown(&view);
    // A rebuild keeps the old view while it builds and diffs the new one.
    let s9 = reset_peak();
    let view2 = View::build(&settings, &effective, &roots);
    let refresh = delta(&view, &view2, false);
    let (rebuild_peak, _) = peak_above(s9);
    drop(refresh);
    let view2_size = freed_by(view2);
    let effective_size = freed_by(effective);
    out.insert(
        "view".into(),
        json!({
            "effective": delta_of(s6, s7),
            "effective_freed": effective_size,
            "view": delta_of(s7, s8),
            "build_peak_bytes": view_build_peak,
            "rebuild_with_old_view_peak_bytes": rebuild_peak,
            "second_view": view2_size,
            "arena_nodes": arena_nodes,
            "rendered_nodes": rendered,
            "arena_id_bytes": id_bytes,
            "max_id_len": max_id,
            "breakdown": view_parts,
        }),
    );

    // --- Open documents: N real source files plus one large file, as the
    // server holds them: the text, its todos, and a buffer entry in the index.
    let mut docs = Documents::default();
    let mut open_paths: Vec<PathBuf> = walked_paths(&index, &roots)
        .into_iter()
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| ["ts", "tsx", "js", "jsx", "py", "go", "rs"].contains(&e))
        })
        .take(a.open)
        .collect();
    let small = open_paths.len();
    if let Some(b) = &a.big {
        open_paths.push(b.clone());
    }
    let s10 = snap();
    let mut text_bytes = 0usize;
    let mut doc_todos = 0usize;
    for (i, path) in open_paths.iter().enumerate() {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        drop(bytes);
        text_bytes += text.len();
        let uri = format!("file://{}", path.display());
        let todos = scan_text(&pat, text.as_bytes(), &path.to_string_lossy());
        doc_todos += todos.len();
        index.set_buffer(BufferEntry {
            uri: uri.clone(),
            path: Some(path.clone()),
            version: i as i32,
            todos: todos.clone(),
        });
        docs.insert(Document {
            uri,
            path: Some(path.clone()),
            version: i as i32,
            text,
            todos,
            scanned: Some(i as i32),
            admitted: true,
        });
    }
    let s11 = snap();
    out.insert(
        "documents".into(),
        json!({
            "small_files": small,
            "big_file": a.big.as_ref().map(|b| b.display().to_string()),
            "text_bytes": text_bytes,
            "todos": doc_todos,
            "held": delta_of(s10, s11),
        }),
    );

    // --- Decorations for the largest open document, and a synthetic
    // 50,000-line buffer with a todo every 10 lines (5,000 ranges).
    let mut decor = serde_json::Map::new();
    let mut measure_decor = |name: &str, text: &str| {
        let s = reset_peak();
        let todos = scan_text(&pat, text.as_bytes(), "a.ts");
        let ranges = decorate(text.as_bytes(), &todos, &settings, &pat);
        let (peak, _) = peak_above(s);
        let n: usize = ranges.values().map(Vec::len).sum();
        let msg = serde_json::to_string(&ranges).map_or(0, |m| m.len());
        let size = freed_by(ranges);
        let todos_size = freed_by(todos);
        decor.insert(
            name.into(),
            json!({ "text_bytes": text.len(), "ranges": n, "ranges_map": size,
                     "todos": todos_size, "peak_bytes": peak, "json_bytes": msg }),
        );
    };
    if let Some(b) = &a.big {
        if let Ok(bytes) = std::fs::read(b) {
            measure_decor("big_file", &String::from_utf8_lossy(&bytes));
        }
    }
    let synthetic: String = (0..50_000)
        .map(|i| {
            if i % 10 == 0 {
                format!("// TODO item {i}\n")
            } else {
                format!("let x{i} = {i};\n")
            }
        })
        .collect();
    measure_decor("synthetic_50k_lines_5k_todos", &synthetic);
    drop(synthetic);
    out.insert("decorations".into(), Value::Object(decor));

    // --- A rescan while the index is live: a second walk plus apply.
    let s12 = reset_peak();
    let outcome2 =
        walk_and_scan(&core, &roots, &pat, fs.clone(), &AtomicBool::new(false)).expect("walk");
    let WalkParts { files, seen } = split(outcome2);
    index.apply_walk(&roots, files, &seen, true);
    drop(seen);
    let (rescan_peak, rescan_peak_real) = peak_above(s12);
    let s13 = snap();
    out.insert(
        "rescan".into(),
        json!({ "peak_bytes": rescan_peak, "peak_reserved": rescan_peak_real,
                "net": delta_of(s12, s13) }),
    );

    // --- Sizes by dropping each component, in dependency order.
    let view_size = freed_by(view);
    let docs_size = freed_by(docs);
    let index_before_buffers = snap();
    for p in &open_paths {
        let _ = index.remove_buffer(&format!("file://{}", p.display()));
    }
    let buffers_size = delta_of(snap(), index_before_buffers);
    let index_size = freed_by(index);
    let admission_size = freed_by(admission);
    let pattern_size = freed_by(pat);
    let settings_size = freed_by((settings, core, fs, roots, walked));
    let remainder = delta_of(base, snap());
    for (name, v, note) in [
        (
            "index (disk results)",
            &index_size,
            format!("{disk_files} files, {todo_count} todos"),
        ),
        (
            "view (arena + rendered nodes + maps)",
            &view_size,
            format!("{arena_nodes} arena nodes, {rendered} rendered"),
        ),
        (
            "open documents (text + todos)",
            &docs_size,
            format!("{} docs, {text_bytes} text bytes", open_paths.len()),
        ),
        ("buffer entries in the index", &buffers_size, String::new()),
        (
            "admission incl. ignore cache",
            &admission_size,
            String::new(),
        ),
        ("pattern (incl. regex caches)", &pattern_size, String::new()),
        ("settings and roots", &settings_size, String::new()),
        ("left after all drops", &remainder, String::new()),
    ] {
        row(&mut rows, name, v, note);
    }
    out.insert(
        "sizes".into(),
        json!({
            "index": index_size, "view": view_size, "documents": docs_size,
            "buffer_entries": buffers_size, "admission": admission_size,
            "pattern": pattern_size, "settings": settings_size, "remainder": remainder,
        }),
    );
    let ib = index_size["bytes"].as_i64().unwrap_or(0) as f64;
    let vb = view_size["bytes"].as_i64().unwrap_or(0) as f64;
    let per = |b: f64, n: usize| if n == 0 { 0.0 } else { b / n as f64 };
    out.insert(
        "per_unit".into(),
        json!({
            "index_per_todo": per(ib, todo_count),
            "index_per_file_with_todos": per(ib, disk_files),
            "view_per_todo": per(vb, todo_count),
            "index_plus_view_per_todo": per(ib + vb, todo_count),
            "index_plus_view_per_admitted_file": per(ib + vb, seen_count),
        }),
    );

    println!("root: {}", a.root.display());
    println!(
        "admitted files: {seen_count}, files with todos: {files_with_todos}, todos: {todo_count}"
    );
    println!("{:<44} {:>12}  notes", "component", "live");
    for (name, b, note) in &rows {
        println!("{name:<44} {:>12}  {note}", mib(*b));
    }
    println!(
        "view build peak {}, rebuild with old view {}, rescan peak {}",
        mib(view_build_peak as i64),
        mib(rebuild_peak as i64),
        mib(rescan_peak as i64)
    );
    println!("JSON {}", Value::Object(out));
}

struct WalkParts {
    files: Vec<clippings_core::model::FileResult>,
    seen: Vec<PathBuf>,
}

fn split(o: clippings_core::walker::WalkOutcome) -> WalkParts {
    WalkParts {
        files: o.files,
        seen: o.seen,
    }
}

/// Paths of the index's disk results, found by walking the roots' seen
/// files would need them again; the index exposes lookups only, so this
/// collects the paths through `effective` in workspace-only mode.
fn walked_paths(index: &Index, roots: &[PathBuf]) -> Vec<PathBuf> {
    let ctx = EffectiveContext {
        mode: clippings_core::config::ScanMode::WorkspaceOnly,
        walked_roots: roots,
        active_uri: None,
    };
    index
        .effective(&ctx)
        .iter()
        .filter_map(|f| f.path.map(Path::to_path_buf))
        .collect()
}

fn cap(s: &str) -> usize {
    s.len()
}

fn opt(s: &Option<String>) -> usize {
    s.as_ref().map_or(0, |s| s.capacity())
}

/// Heap bytes of a `HashMap`'s table: one key-value slot plus one control
/// byte per bucket, as hashbrown lays it out (an estimate; the allocator
/// numbers above are exact).
fn table<K, V>(capacity: usize) -> usize {
    let buckets = if capacity == 0 {
        0
    } else {
        (capacity * 8 / 7).next_power_of_two()
    };
    buckets * (std::mem::size_of::<(K, V)>() + 1)
}

fn icon_heap(i: &Option<IconDescriptor>) -> usize {
    match i {
        Some(IconDescriptor::Codicon { name, colour }) => name.capacity() + opt(colour),
        Some(IconDescriptor::Octicon { name, colour }) => name.capacity() + colour.capacity(),
        Some(IconDescriptor::TodoTree { colour, .. }) | Some(IconDescriptor::Check { colour }) => {
            colour.capacity()
        }
        _ => 0,
    }
}

/// Where a built view's bytes go, from string lengths and capacities:
/// how much is node ID text (stored in several maps) and how much is
/// everything else.
fn view_breakdown(v: &View) -> Value {
    let mut ids = 0usize;
    let mut other_strings = 0usize;
    // Arena: one Node per placed node, its ID and its other strings.
    for n in &v.arena.nodes {
        ids += n.id.capacity();
        other_strings += n.name.capacity()
            + n.path.as_ref().map_or(0, |p| p.as_os_str().len())
            + opt(&n.uri)
            + opt(&n.key)
            + opt(&n.sub_tag)
            + opt(&n.path_label)
            + n.children.capacity() * 8
            + n.status.as_ref().map_or(0, |s| s.0.capacity() + opt(&s.2))
            + n.todo.as_ref().map_or(0, |t| {
                t.uri.capacity()
                    + t.tag.capacity()
                    + opt(&t.sub_tag)
                    + t.before.capacity()
                    + t.after.capacity()
                    + t.text.capacity()
            });
    }
    let arena_structs = v.arena.nodes.capacity() * std::mem::size_of::<Node>();
    // by_id: a second copy of every ID as the key.
    let by_id_keys: usize = v.arena.by_id.keys().map(|k| cap(k)).sum();
    let by_id_table = table::<String, usize>(v.arena.by_id.capacity());
    // Rendered nodes: key ID, ViewNode.id, and the rendered strings.
    let mut rendered_ids = 0usize;
    let mut rendered_other = 0usize;
    for (k, n) in &v.nodes {
        rendered_ids += cap(k) + n.id.capacity();
        rendered_other += n.label.capacity()
            + opt(&n.description)
            + opt(&n.tooltip)
            + opt(&n.context_value)
            + opt(&n.resource_uri)
            + icon_heap(&n.icon)
            + match &n.command {
                Some(NodeCommand::Reveal { uri, .. }) => uri.capacity(),
                Some(NodeCommand::OpenUrl { url }) => url.capacity(),
                None => 0,
            };
    }
    let nodes_table = table::<String, ViewNode>(v.nodes.capacity());
    let mut children_ids = 0usize;
    for (k, kids) in &v.children {
        children_ids += opt(k) + kids.iter().map(|c| c.capacity()).sum::<usize>();
        children_ids += kids.capacity() * std::mem::size_of::<String>();
    }
    let children_table = table::<Option<String>, Vec<String>>(v.children.capacity());
    let parents_ids: usize = v.parents.iter().map(|(k, p)| k.capacity() + opt(p)).sum();
    let parents_table = table::<String, Option<String>>(v.parents.capacity());
    let shaped = v.shaped.visible.capacity()
        + v.shaped.counts.capacity() * 8
        + v.shaped
            .children
            .iter()
            .map(|c| c.capacity() * 8 + 24)
            .sum::<usize>()
        + v.shaped.top.capacity() * 8
        + v.shaped.parent.capacity() * 16
        + v.shaped.compact_label.iter().map(opt).sum::<usize>()
        + v.shaped.compact_label.capacity() * 24;
    json!({
        "id_text": {
            "arena_node_id": ids,
            "by_id_key": by_id_keys,
            "rendered_key_and_viewnode_id": rendered_ids,
            "children_map": children_ids,
            "parents_map": parents_ids,
            "total": ids + by_id_keys + rendered_ids + children_ids + parents_ids,
        },
        "arena_other_strings": other_strings,
        "rendered_other_strings": rendered_other,
        "structs_and_tables": arena_structs + by_id_table + nodes_table + children_table
            + parents_table + shaped,
        "node_size": std::mem::size_of::<Node>(),
        "view_node_size": std::mem::size_of::<ViewNode>(),
    })
}
