//! The language server (spec sections 5.10, 5.11, 6): one scheduler owns
//! the index, open documents and view, and turns client messages, worker
//! results and timers into outgoing notifications.

pub mod git;
pub mod main_loop;
pub mod watch;

use crate::admission::Admission;
use crate::config::ScanMode;
use crate::decorations::decorate;
use crate::documents::{apply_changes, Document, Documents};
use crate::fs::Fs;
use crate::index::{BufferEntry, EffectiveContext, Index};
use crate::labels::unexpected_placeholder;
use crate::navigate::navigate;
use crate::pattern::{self, ScanPattern};
use crate::protocol::{self as p, method, FileEvent, InitializeParams, StatusParams};
use crate::roots::{resolve_roots, walked_roots, Roots};
use crate::scanner::{scan_file, scan_text};
use crate::settings::{Settings, StatusBarMode};
use crate::status::summarize;
use crate::styles::{colour_warnings, Resolver};
use crate::uri::{scheme, uri_to_path};
use crate::view::delta::delta;
use crate::view::export::{export_content, export_path};
use crate::view::render::View;
use crate::walker::{walk_and_scan, WalkOutcome};
use crossbeam_channel::{unbounded, Receiver, Sender};
use lsp_server::{Message, Notification, Request, RequestId, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// View rebuilds coalesce for this long.
pub const VIEW_DELAY: Duration = Duration::from_millis(50);
/// File events coalesce for this long.
pub const EVENT_DELAY: Duration = Duration::from_millis(50);
/// A buffer is rescanned for the tree after this long without edits.
pub const BUFFER_DELAY: Duration = Duration::from_millis(150);

pub type Env = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Work finished off the scheduler thread.
pub enum Work {
    Walk {
        generation: u64,
        roots: Vec<PathBuf>,
        outcome: Result<WalkOutcome, String>,
    },
    Git {
        folder: PathBuf,
        head: Option<String>,
    },
    /// Events from the notify fallback; `None` means the watcher failed.
    Files(Option<Vec<FileEvent>>),
}

pub struct Server {
    out: Sender<Message>,
    fs: Arc<dyn Fs>,
    env: Env,
    settings: Settings,
    folders: Vec<PathBuf>,
    roots: Roots,
    walked: Vec<PathBuf>,
    pattern: Arc<ScanPattern>,
    /// A configuration error: invalid regex, glob or root.
    error: Option<String>,
    /// The last full walk's error, cleared by the next walk that succeeds.
    walk_error: Option<String>,
    admission: Arc<Admission>,
    index: Index,
    docs: Documents,
    view: View,
    active_uri: Option<String>,
    instance: String,
    scan_generation: u64,
    /// Paths (files, or directories standing for everything below them)
    /// that file events updated while the current full walk ran. The walk
    /// may have read them before the event, so applying it leaves them alone.
    touched: HashSet<PathBuf>,
    cancel: Arc<AtomicBool>,
    scanning: bool,
    interrupted: bool,
    needs_scan: bool,
    style_generation: u64,
    sent_styles: HashSet<String>,
    view_due: Option<Instant>,
    whole_tree: bool,
    buffer_due: BTreeMap<String, Instant>,
    decorations_due: BTreeMap<String, Instant>,
    events: Vec<FileEvent>,
    events_due: Option<Instant>,
    git_due: Option<Instant>,
    periodic_due: Option<Instant>,
    git_heads: HashMap<PathBuf, String>,
    git_in_flight: HashSet<PathBuf>,
    dynamic_watchers: bool,
    registered: bool,
    next_request: i32,
    notify: Option<watch::NotifyWatcher>,
    work_tx: Sender<Work>,
    pub work_rx: Receiver<Work>,
    last_status: Option<StatusParams>,
    /// Why the last `clippings/configure` payload could not be read.
    config_warning: Option<String>,
}

fn instance_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{:x}-{:x}", std::process::id(), nanos)
}

/// Runs a per-document computation, logging a panic and returning `None`
/// so the caller keeps the document's previous state (spec 5.10).
fn guarded<T>(uri: &str, what: &str, f: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(f))
        .map_err(|_| tracing::warn!("{what} of {uri} panicked; keeping its previous state"))
        .ok()
}

/// `now + d`, or `None` when that is past what `Instant` can hold: the
/// timer never fires.
fn later(now: Instant, d: Duration) -> Option<Instant> {
    now.checked_add(d)
}

fn folder_paths(folders: &[p::WorkspaceFolder]) -> Vec<PathBuf> {
    folders
        .iter()
        .filter(|f| scheme(&f.uri).as_deref() == Some("file"))
        .filter_map(|f| uri_to_path(&f.uri))
        .collect()
}

impl Server {
    pub fn new(
        out: Sender<Message>,
        fs: Arc<dyn Fs>,
        env: Env,
        params: &InitializeParams,
    ) -> Server {
        let settings = params
            .initialization_options
            .clone()
            .unwrap_or_default()
            .settings;
        let folders = folder_paths(params.workspace_folders.as_deref().unwrap_or(&[]));
        let (work_tx, work_rx) = unbounded();
        let core = settings.core();
        let (pattern, error) = match pattern::build(&core) {
            Ok(pt) => (pt, None),
            Err(e) => (
                pattern::build(&crate::config::CoreConfig::default()).expect("default pattern"),
                Some(e.to_string()),
            ),
        };
        let mut s = Server {
            out,
            fs: fs.clone(),
            env,
            settings,
            folders,
            roots: Roots {
                tree_roots: vec![],
                scan_roots: vec![],
                from_root_folder: false,
            },
            walked: vec![],
            pattern: Arc::new(pattern),
            error,
            walk_error: None,
            // A placeholder until `rebuild_scan_state`, which reports the
            // user's invalid globs in the status instead of panicking.
            admission: Arc::new(
                Admission::new(&crate::config::CoreConfig::default(), vec![], fs)
                    .expect("default admission"),
            ),
            index: Index::new(),
            docs: Documents::default(),
            view: View::default(),
            active_uri: None,
            instance: instance_id(),
            scan_generation: 0,
            touched: HashSet::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            scanning: false,
            interrupted: false,
            needs_scan: false,
            style_generation: 0,
            sent_styles: HashSet::new(),
            view_due: None,
            whole_tree: true,
            buffer_due: BTreeMap::new(),
            decorations_due: BTreeMap::new(),
            events: Vec::new(),
            events_due: None,
            git_due: None,
            periodic_due: None,
            git_heads: HashMap::new(),
            git_in_flight: HashSet::new(),
            dynamic_watchers: params.dynamic_watchers(),
            registered: false,
            next_request: 0,
            notify: None,
            work_tx,
            work_rx,
            last_status: None,
            config_warning: None,
        };
        s.rebuild_scan_state();
        s
    }

    fn send_notification(&self, method: &str, params: impl Serialize) {
        let _ = self
            .out
            .send(Notification::new(method.to_string(), params).into());
    }

    fn send_request(&mut self, method: &str, params: impl Serialize) {
        self.next_request += 1;
        let id = RequestId::from(format!("clippings-{}", self.next_request));
        let _ = self
            .out
            .send(Request::new(id, method.to_string(), params).into());
    }

    /// Called once the client has sent `initialized`.
    pub fn start(&mut self, now: Instant) {
        self.update_watchers();
        self.send_notification(
            method::STYLES,
            p::StylesParams {
                generation: 0,
                reset: true,
                styles: BTreeMap::new(),
            },
        );
        if self.settings.tree.scan_at_startup {
            self.full_rescan();
        } else {
            self.needs_scan = true;
        }
        self.reset_timers(now);
        self.schedule_view(now, true);
        self.send_status();
    }

    fn rebuild_scan_state(&mut self) {
        let core = self.settings.core();
        match pattern::build(&core) {
            Ok(pt) => {
                self.pattern = Arc::new(pt);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        let env = self.env.clone();
        match resolve_roots(&self.folders, &core, &|n| env(n)) {
            Ok(r) => self.roots = r,
            Err(e) => self.error = Some(e.to_string()),
        }
        self.walked = walked_roots(&self.roots, core.scan_mode);
        match Admission::new(&core, self.walked.clone(), self.fs.clone()) {
            Ok(a) => self.admission = Arc::new(a),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn update_watchers(&mut self) {
        if self.dynamic_watchers {
            if self.registered {
                self.send_request(method::UNREGISTER_CAPABILITY, watch::unregister_params());
                self.registered = false;
            }
            if !self.walked.is_empty() {
                self.send_request(
                    method::REGISTER_CAPABILITY,
                    watch::register_params(&self.walked),
                );
                self.registered = true;
            }
        } else {
            let tx = self.work_tx.clone();
            let canonical = watch::canonical_roots(&self.walked);
            self.notify = watch::NotifyWatcher::new(&self.walked, move |mut e| {
                if let Some(events) = e.as_mut() {
                    watch::rebase_events(events, &canonical);
                }
                let _ = tx.send(Work::Files(e));
            })
            .ok();
        }
    }

    /// A full rescan that also refreshes every open document.
    fn rescan_all(&mut self, now: Instant) {
        for uri in self.docs.uris() {
            self.rescan_buffer(&uri, now, true);
        }
        self.full_rescan();
    }

    fn full_rescan(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.scan_generation += 1;
        self.touched.clear();
        self.scanning = true;
        self.interrupted = false;
        self.needs_scan = false;
        let (generation, roots, core) = (
            self.scan_generation,
            self.walked.clone(),
            self.settings.core(),
        );
        let (pattern, fs, cancel, tx) = (
            self.pattern.clone(),
            self.fs.clone(),
            self.cancel.clone(),
            self.work_tx.clone(),
        );
        std::thread::spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                walk_and_scan(&core, &roots, &pattern, fs, &cancel)
            }))
            .map_err(|_| "scan panicked".to_string())
            .and_then(|r| r.map_err(|e| e.to_string()));
            let _ = tx.send(Work::Walk {
                generation,
                roots,
                outcome,
            });
        });
        self.send_status();
    }

    fn reset_timers(&mut self, now: Instant) {
        let g = self.settings.general.automatic_git_refresh_interval;
        let m = self.settings.general.periodic_refresh_interval;
        self.git_due = (g > 0)
            .then(|| later(now, Duration::from_secs(g)))
            .flatten();
        self.periodic_due = (m > 0)
            .then(|| later(now, Duration::from_secs(m.saturating_mul(60))))
            .flatten();
    }

    fn schedule_view(&mut self, now: Instant, whole_tree: bool) {
        self.whole_tree |= whole_tree;
        let due = now + VIEW_DELAY;
        self.view_due = Some(self.view_due.map_or(due, |d| d.min(due)));
    }

    fn buffer_admitted(&self, uri: &str, path: Option<&Path>) -> bool {
        let sch = scheme(uri).unwrap_or_default();
        self.settings.general.schemes.contains(&sch)
            && self.admission.admits_buffer(path, &self.roots.tree_roots)
    }

    fn scan_document(&self, doc: &Document) -> Vec<crate::model::Todo> {
        #[cfg(test)]
        tests::maybe_panic(&doc.uri);
        if !doc.admitted {
            return Vec::new();
        }
        let path = doc
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| doc.uri.clone());
        scan_text(&self.pattern, doc.text.as_bytes(), &path)
    }

    /// Rescans an open buffer and feeds the index when auto-refresh is on or
    /// `force` is set: explicit, periodic, git and overflow rescans refresh
    /// open documents whatever `tree.autoRefresh` says, as todo-tree's
    /// `rebuild` does. Returns false when the scan panicked, leaving the
    /// previous state.
    fn rescan_buffer(&mut self, uri: &str, now: Instant, force: bool) -> bool {
        let Some(doc) = self.docs.get(uri) else {
            return true;
        };
        let Some(todos) = guarded(uri, "scan", || self.scan_document(doc)) else {
            // The kept todos may predate a pattern change: decorations rescan.
            if let Some(d) = self.docs.get_mut(uri) {
                d.scanned = None;
            }
            return false;
        };
        let entry =
            (doc.admitted && (force || self.settings.tree.auto_refresh)).then(|| BufferEntry {
                uri: doc.uri.clone(),
                path: doc.path.clone(),
                version: doc.version,
                todos: todos.clone(),
            });
        if let Some(d) = self.docs.get_mut(uri) {
            d.todos = todos;
            d.scanned = Some(d.version);
        }
        if let Some(entry) = entry {
            self.index.set_buffer(entry);
            self.schedule_view(now, false);
        }
        true
    }

    /// Recomputes each open document's admission after the rules or roots
    /// changed. A document whose admission flips gets its decorations
    /// recomputed, cleared when it is no longer admitted, and leaves the
    /// tree when it is no longer admitted.
    fn readmit_documents(&mut self, now: Instant) {
        for uri in self.docs.uris() {
            let Some(d) = self.docs.get(&uri) else {
                continue;
            };
            let admitted = self.buffer_admitted(&d.uri, d.path.as_deref());
            if admitted == d.admitted {
                continue;
            }
            if let Some(d) = self.docs.get_mut(&uri) {
                d.admitted = admitted;
                d.scanned = None;
            }
            if !admitted {
                self.index.remove_buffer(&uri);
                self.schedule_view(now, false);
            }
            self.decorations_due.insert(uri, now);
        }
    }

    /// Records that a file event updated `path` (and everything below it),
    /// so the running full walk does not overwrite it.
    fn touch(&mut self, path: &Path) {
        if self.scanning {
            self.touched.insert(path.to_path_buf());
        }
    }

    fn rescan_disk_file(&mut self, path: &Path) {
        self.touch(path);
        match scan_file(self.fs.as_ref(), &self.pattern, path) {
            Ok(Some(todos)) => self.index.set_disk(path.to_path_buf(), todos),
            _ => self.index.remove_disk(path),
        }
    }

    /// The files a rewalk of `dir` visits, pruning directories as a full
    /// walk does, or `None` when a full walk would never enter `dir`.
    fn rewalk_candidates(&self, dir: &Path) -> Option<Vec<PathBuf>> {
        if !self.walked.iter().any(|r| dir.starts_with(r))
            || self.admission.prunes_dir(dir)
            || self.admission.inside_pruned_dir(dir)
        {
            return None;
        }
        let admission = self.admission.clone();
        let files = ignore::WalkBuilder::new(dir)
            .standard_filters(false)
            .filter_entry(move |e| {
                e.depth() == 0
                    || !e.file_type().is_some_and(|t| t.is_dir())
                    || !admission.prunes_dir(e.path())
            })
            .build()
            .flatten()
            .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
            .map(|e| e.into_path())
            .collect();
        Some(files)
    }

    fn rewalk_dir(&mut self, dir: &Path) {
        let Some(candidates) = self.rewalk_candidates(dir) else {
            return;
        };
        let mut files = Vec::new();
        let mut seen = Vec::new();
        for path in candidates {
            if !self.admission.admits_disk(&path) {
                continue;
            }
            if let Ok(Some(todos)) = scan_file(self.fs.as_ref(), &self.pattern, &path) {
                seen.push(path.clone());
                if !todos.is_empty() {
                    files.push(crate::model::FileResult { path, todos });
                }
            }
        }
        self.touch(dir);
        self.index
            .apply_walk(&[dir.to_path_buf()], files, &seen, true);
    }

    fn process_events(&mut self, now: Instant) {
        let events = std::mem::take(&mut self.events);
        // One entry per path, in path order so a directory comes before its
        // contents: whether any event deleted it, whether any created it,
        // and whether it exists after its last event.
        let mut paths: BTreeMap<PathBuf, (bool, bool, bool)> = BTreeMap::new();
        for e in events {
            let Some(path) = uri_to_path(&e.uri) else {
                continue;
            };
            let deleted = e.kind == p::FILE_DELETED;
            let entry = paths.entry(path).or_insert((false, false, false));
            entry.0 |= deleted;
            entry.1 |= e.kind == p::FILE_CREATED;
            entry.2 = !deleted;
        }
        let mut full = false;
        let mut rewalked: Vec<PathBuf> = Vec::new();
        for (path, (deleted, created, exists)) in paths {
            // Drop events a walk would never reach before any stat, so
            // `node_modules` and the like cost nothing.
            if !self.walked.iter().any(|r| path.starts_with(r))
                || self.admission.inside_pruned_dir(&path)
            {
                continue;
            }
            // Before the rewalk skip: a rewalk does not reload ignore rules.
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if self.admission.respects_ignore_files()
                && matches!(name.as_str(), ".gitignore" | ".ignore" | ".rgignore")
            {
                self.admission.clear_ignore_cache();
                full = true;
                continue;
            }
            if rewalked.iter().any(|d| path.starts_with(d)) {
                continue;
            }
            if deleted {
                self.touch(&path);
                self.index.remove_disk_prefix(&path);
            }
            if !exists {
                continue;
            }
            if self.fs.is_dir(&path) {
                // A Changed directory needs nothing: changes below it arrive
                // as their own events. Only a created one is rewalked.
                if created && !self.admission.prunes_dir(&path) {
                    self.rewalk_dir(&path);
                    rewalked.push(path);
                }
            } else if self.admission.admits_disk(&path) {
                self.rescan_disk_file(&path);
            } else {
                self.touch(&path);
                self.index.remove_disk(&path);
            }
        }
        if full {
            self.full_rescan();
        }
        self.schedule_view(now, false);
    }

    fn rebuild_view(&mut self) {
        let active = self.active_uri.clone();
        let ctx = EffectiveContext {
            mode: self.settings.tree.scan_mode,
            walked_roots: &self.walked,
            active_uri: active.as_deref(),
        };
        let files = self.index.effective(&ctx);
        let built = catch_unwind(AssertUnwindSafe(|| {
            View::build(&self.settings, &files, &self.roots.tree_roots)
        }));
        let Ok(new) = built else {
            tracing::error!("view build panicked; keeping the previous view");
            return;
        };
        let refresh = delta(&self.view, &new, std::mem::take(&mut self.whole_tree));
        self.view = new;
        if !refresh.is_empty() {
            self.send_notification(method::TREE_CHANGED, p::TreeChangedParams { refresh });
        }
        self.send_status();
    }

    fn send_status(&mut self) {
        let active_path = self.active_uri.as_deref().and_then(uri_to_path);
        let summary = summarize(&self.view, &self.settings, active_path.as_deref());
        let mut warnings = colour_warnings(&self.settings);
        warnings.extend(self.config_warning.clone());
        let bad: Vec<String> = [
            &self.settings.tree.label_format,
            &self.settings.tree.tooltip_format,
        ]
        .iter()
        .filter_map(|t| unexpected_placeholder(t))
        .collect();
        if !bad.is_empty() {
            warnings.push(format!("Unexpected placeholders ({})", bad.join(",")));
        }
        let status = StatusParams {
            instance: self.instance.clone(),
            scanning: self.scanning,
            interrupted: self.interrupted,
            needs_scan: self.needs_scan,
            error: self.error.clone().or_else(|| self.walk_error.clone()),
            warnings,
            status_bar: summary.status_bar,
            badge: summary.badge,
            view_title: summary.view_title,
            has_sub_tags: self.view.has_sub_tags,
            is_empty: self.view.is_empty,
        };
        if self.last_status.as_ref() != Some(&status) {
            self.send_notification(method::STATUS, &status);
            self.last_status = Some(status);
        }
    }

    fn send_decorations(&mut self, uri: &str) {
        let Some(doc) = self.docs.get(uri) else {
            return;
        };
        let computed = guarded(uri, "decoration", || {
            // The buffer scan of this version, when there is one.
            let fresh = (doc.scanned != Some(doc.version)).then(|| self.scan_document(doc));
            let ranges = if doc.admitted {
                let todos = fresh.as_deref().unwrap_or(&doc.todos);
                decorate(doc.text.as_bytes(), todos, &self.settings, &self.pattern)
            } else {
                BTreeMap::new()
            };
            let resolver = Resolver::new(&self.settings);
            let new_styles: BTreeMap<String, _> = ranges
                .keys()
                .filter(|k| !self.sent_styles.contains(*k))
                .map(|k| (k.clone(), resolver.style(k)))
                .collect();
            (fresh, ranges, new_styles)
        });
        let Some((fresh, ranges, new_styles)) = computed else {
            return;
        };
        let (doc_uri, version) = (doc.uri.clone(), doc.version);
        if let (Some(todos), Some(d)) = (fresh, self.docs.get_mut(uri)) {
            d.todos = todos;
            d.scanned = Some(d.version);
        }
        if !new_styles.is_empty() {
            self.sent_styles.extend(new_styles.keys().cloned());
            self.send_notification(
                method::STYLES,
                p::StylesParams {
                    generation: self.style_generation,
                    reset: false,
                    styles: new_styles,
                },
            );
        }
        self.send_notification(
            method::DECORATIONS,
            p::DecorationsParams {
                uri: doc_uri,
                version,
                generation: self.style_generation,
                ranges,
            },
        );
    }

    fn configure(&mut self, new: Settings, now: Instant) {
        let changes = new.changes_from(&self.settings);
        let old_walked = self.walked.clone();
        self.settings = new;
        if changes.rescan {
            self.rebuild_scan_state();
            if self.walked != old_walked {
                self.update_watchers();
            }
            self.readmit_documents(now);
            self.rescan_all(now);
        }
        if changes.styles {
            self.style_generation += 1;
            self.sent_styles.clear();
            self.send_notification(
                method::STYLES,
                p::StylesParams {
                    generation: self.style_generation,
                    reset: true,
                    styles: BTreeMap::new(),
                },
            );
            for uri in self.docs.uris() {
                self.decorations_due.insert(uri, now);
            }
        }
        if changes.view {
            self.schedule_view(now, changes.view_mode);
        }
        if changes.timers {
            self.reset_timers(now);
        }
        if changes.status {
            self.send_status();
        }
    }

    fn params<P: DeserializeOwned>(value: serde_json::Value) -> Option<P> {
        serde_json::from_value(value)
            .map_err(|e| tracing::warn!("bad params: {e}"))
            .ok()
    }

    pub fn handle_notification(&mut self, n: Notification, now: Instant) {
        match n.method.as_str() {
            method::DID_OPEN => {
                let Some(params) = Self::params::<p::DidOpenParams>(n.params) else {
                    return;
                };
                let item = params.text_document;
                let path = uri_to_path(&item.uri);
                let admitted = self.buffer_admitted(&item.uri, path.as_deref());
                let uri = item.uri.clone();
                self.docs.insert(Document {
                    uri: item.uri,
                    path,
                    version: item.version,
                    text: item.text,
                    todos: Vec::new(),
                    scanned: None,
                    admitted,
                });
                self.rescan_buffer(&uri, now, false);
                self.decorations_due.insert(uri, now);
            }
            method::DID_CHANGE => {
                let Some(params) = Self::params::<p::DidChangeParams>(n.params) else {
                    return;
                };
                let uri = params.text_document.uri;
                let Some(doc) = self.docs.get_mut(&uri) else {
                    return;
                };
                apply_changes(&mut doc.text, &params.content_changes);
                doc.version = params.text_document.version;
                self.buffer_due.insert(uri.clone(), now + BUFFER_DELAY);
                let delay = Duration::from_millis(self.settings.highlights.highlight_delay);
                match later(now, delay) {
                    Some(due) => self.decorations_due.insert(uri, due),
                    None => self.decorations_due.remove(&uri),
                };
            }
            method::DID_CLOSE => {
                let Some(params) = Self::params::<p::DidCloseParams>(n.params) else {
                    return;
                };
                let uri = params.text_document.uri;
                self.docs.remove(&uri);
                self.buffer_due.remove(&uri);
                self.decorations_due.remove(&uri);
                if let Some(path) = self.index.remove_buffer(&uri) {
                    if self.admission.admits_disk(&path) {
                        self.rescan_disk_file(&path);
                    }
                }
                self.schedule_view(now, false);
            }
            method::DID_CHANGE_WATCHED_FILES => {
                let Some(params) = Self::params::<p::DidChangeWatchedFilesParams>(n.params) else {
                    return;
                };
                if self.settings.tree.auto_refresh {
                    self.events.extend(params.changes);
                    self.events_due.get_or_insert(now + EVENT_DELAY);
                }
            }
            method::DID_CHANGE_WORKSPACE_FOLDERS => {
                let Some(params) = Self::params::<p::DidChangeWorkspaceFoldersParams>(n.params)
                else {
                    return;
                };
                let removed = folder_paths(&params.event.removed);
                self.folders.retain(|f| !removed.contains(f));
                self.folders.extend(folder_paths(&params.event.added));
                self.rebuild_scan_state();
                self.update_watchers();
                self.readmit_documents(now);
                self.rescan_all(now);
                self.schedule_view(now, true);
            }
            method::CONFIGURE => match serde_json::from_value::<Settings>(n.params) {
                Ok(s) => {
                    let cleared = self.config_warning.take().is_some();
                    self.configure(s, now);
                    if cleared {
                        self.send_status();
                    }
                }
                Err(e) => {
                    tracing::warn!("bad configuration: {e}");
                    self.config_warning = Some(format!(
                        "Invalid configuration, keeping the previous one: {e}"
                    ));
                    self.send_status();
                }
            },
            method::ACTIVE_EDITOR => {
                let Some(params) = Self::params::<p::ActiveEditorParams>(n.params) else {
                    return;
                };
                self.active_uri = params.uri;
                if self.settings.tree.scan_mode == ScanMode::CurrentFile {
                    self.schedule_view(now, false);
                }
                if self.settings.general.status_bar == StatusBarMode::CurrentFile {
                    self.send_status();
                }
            }
            method::RESCAN => self.rescan_all(now),
            method::STOP_SCAN => self.cancel.store(true, Ordering::Relaxed),
            _ => {}
        }
    }

    pub fn handle_request(&mut self, r: Request) -> Response {
        let id = r.id.clone();
        let ok = |v: serde_json::Value| Response::new_ok(id.clone(), v);
        let bad = |m: &str| {
            Response::new_err(
                id.clone(),
                lsp_server::ErrorCode::InvalidParams as i32,
                m.to_string(),
            )
        };
        match r.method.as_str() {
            method::CHILDREN => match Self::params::<p::ChildrenParams>(r.params) {
                Some(q) => ok(serde_json::to_value(p::ChildrenResult {
                    nodes: self.view.children_of(q.parent.as_deref()),
                })
                .unwrap()),
                None => bad("invalid params"),
            },
            method::FIND => match Self::params::<p::FindParams>(r.params) {
                Some(q) => ok(serde_json::to_value(p::FindResult {
                    paths: self.view.find(&q.uri, q.line),
                })
                .unwrap()),
                None => bad("invalid params"),
            },
            method::NAVIGATE => match Self::params::<p::NavigateParams>(r.params) {
                Some(q) => {
                    let ranges = self.docs.get(&q.uri).and_then(|d| {
                        navigate(d.text.as_bytes(), &self.pattern, &q.positions, q.direction)
                    });
                    ok(serde_json::to_value(p::NavigateResult { ranges }).unwrap())
                }
                None => bad("invalid params"),
            },
            method::EXPORT => {
                let env = self.env.clone();
                let home = env("HOME").or_else(|| env("USERPROFILE"));
                let path = export_path(
                    &self.settings.general.export_path,
                    &|n| env(n),
                    home.as_deref(),
                    &chrono::Local::now(),
                );
                let content = export_content(&self.view, self.settings.view().tags_only, &path);
                ok(serde_json::to_value(p::ExportResult { path, content }).unwrap())
            }
            _ => Response::new_err(
                id.clone(),
                lsp_server::ErrorCode::MethodNotFound as i32,
                format!("unknown method {}", r.method),
            ),
        }
    }

    pub fn handle_work(&mut self, w: Work, now: Instant) {
        match w {
            Work::Walk {
                generation,
                roots,
                outcome,
            } => {
                if generation != self.scan_generation {
                    return;
                }
                self.scanning = false;
                let touched = std::mem::take(&mut self.touched);
                match outcome {
                    Ok(o) => {
                        self.interrupted = o.cancelled;
                        self.walk_error = None;
                        if touched.is_empty() {
                            self.index
                                .apply_walk(&roots, o.files, &o.seen, !o.cancelled);
                        } else {
                            self.index.apply_walk_except(
                                &roots,
                                o.files,
                                &o.seen,
                                !o.cancelled,
                                |p| p.ancestors().any(|a| touched.contains(a)),
                            );
                        }
                    }
                    Err(e) => self.walk_error = Some(e),
                }
                self.schedule_view(now, false);
                self.send_status();
            }
            Work::Git { folder, head } => {
                self.git_in_flight.remove(&folder);
                if let Some(h) = head {
                    let changed = self.git_heads.get(&folder).is_some_and(|old| *old != h);
                    self.git_heads.insert(folder, h);
                    if changed {
                        self.rescan_all(now);
                    }
                }
            }
            Work::Files(Some(events)) => {
                if self.settings.tree.auto_refresh {
                    self.events.extend(events);
                    self.events_due.get_or_insert(now + EVENT_DELAY);
                }
            }
            // Like any file event, an overflow updates the tree only with
            // auto-refresh on (spec 5.9).
            Work::Files(None) if self.settings.tree.auto_refresh => {
                tracing::warn!("file watcher failed; rescanning");
                self.rescan_all(now);
            }
            Work::Files(None) => tracing::warn!("file watcher failed; auto-refresh is off"),
        }
    }

    /// The earliest pending timer.
    pub fn next_deadline(&self) -> Option<Instant> {
        [
            self.view_due,
            self.events_due,
            self.git_due,
            self.periodic_due,
        ]
        .into_iter()
        .flatten()
        .chain(self.buffer_due.values().copied())
        .chain(self.decorations_due.values().copied())
        .min()
    }

    /// Runs every timer due at `now`.
    pub fn tick(&mut self, now: Instant) {
        if self.events_due.is_some_and(|d| d <= now) {
            self.events_due = None;
            self.process_events(now);
        }
        let due: Vec<String> = self
            .buffer_due
            .iter()
            .filter(|(_, d)| **d <= now)
            .map(|(u, _)| u.clone())
            .collect();
        for uri in due {
            self.buffer_due.remove(&uri);
            self.rescan_buffer(&uri, now, false);
        }
        let due: Vec<String> = self
            .decorations_due
            .iter()
            .filter(|(_, d)| **d <= now)
            .map(|(u, _)| u.clone())
            .collect();
        for uri in due {
            self.decorations_due.remove(&uri);
            self.send_decorations(&uri);
        }
        if self.periodic_due.is_some_and(|d| d <= now) {
            self.rescan_all(now);
            let minutes = self.settings.general.periodic_refresh_interval.max(1);
            self.periodic_due = later(now, Duration::from_secs(minutes.saturating_mul(60)));
        }
        if self.git_due.is_some_and(|d| d <= now) {
            for folder in self.folders.clone() {
                if self.git_in_flight.insert(folder.clone()) {
                    let tx = self.work_tx.clone();
                    std::thread::spawn(move || {
                        let head = git::head(&folder);
                        let _ = tx.send(Work::Git { folder, head });
                    });
                }
            }
            let seconds = self.settings.general.automatic_git_refresh_interval.max(1);
            self.git_due = later(now, Duration::from_secs(seconds));
        }
        if self.view_due.is_some_and(|d| d <= now) {
            self.view_due = None;
            self.rebuild_view();
        }
    }

    /// After a panic in a handler: the index may be half-updated, so drop it
    /// and rebuild it with a full rescan. A document whose scan panics keeps
    /// its previous todos, as in `rescan_buffer`.
    pub fn recover(&mut self, now: Instant) {
        let old = std::mem::take(&mut self.index);
        for uri in self.docs.uris() {
            if !self.rescan_buffer(&uri, now, true) {
                if let Some(entry) = old.buffer(&uri) {
                    self.index.set_buffer(entry.clone());
                }
            }
        }
        self.full_rescan();
        self.schedule_view(now, true);
    }
}

#[cfg(test)]
mod tests;
