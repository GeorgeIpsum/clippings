// Wire types (spec section 6): a mirror of crates/clippings-core/src/protocol.rs
// and the Rust types it uses (settings.rs, position.rs, styles.rs,
// status.rs, view/render.rs, navigate.rs). Field names follow the serde
// attributes on the Rust side. Keep the two files in step.

export const PROTOCOL_VERSION = 1;

export const Method = {
  configure: 'clippings/configure',
  activeEditor: 'clippings/activeEditor',
  rescan: 'clippings/rescan',
  stopScan: 'clippings/stopScan',
  children: 'clippings/children',
  find: 'clippings/find',
  navigate: 'clippings/navigate',
  export: 'clippings/export',
  treeChanged: 'clippings/treeChanged',
  styles: 'clippings/styles',
  decorations: 'clippings/decorations',
  status: 'clippings/status',
} as const;

// ---- position.rs ----

/** 0-based line and 0-based UTF-16 column. */
export interface Position {
  line: number;
  character: number;
}

/** A half-open range of positions. */
export interface Range {
  start: Position;
  end: Position;
}

// ---- settings.rs and config.rs ----

export type RevealBehaviour = 'start of line' | 'start of todo' | 'end of todo';
export type StatusBarMode = 'none' | 'total' | 'tags' | 'top three' | 'current file';
export type ScanMode = 'workspace' | 'workspace only' | 'open files' | 'current file';
export type UseBuiltInExcludes = 'none' | 'file excludes' | 'search excludes' | 'file and search excludes';

/** Per-tag highlight attributes (`customHighlight.<key>` and `defaultHighlight`). */
export interface Attributes {
  type?: string;
  foreground?: string;
  background?: string;
  opacity?: number;
  rulerColour?: string;
  rulerOpacity?: number;
  /** A lane number or one of `none`, `left`, `center`, `right`, `full`. */
  rulerLane?: number | string;
  borderRadius?: string;
  fontStyle?: string;
  fontWeight?: string;
  textDecoration?: string;
  gutterIcon?: boolean;
  icon?: string;
  iconColour?: string;
  /** US spelling, checked before `iconColour` as in todo-tree. */
  iconColor?: string;
  hideFromTree?: boolean;
  hideFromStatusBar?: boolean;
  hideFromActivityBar?: boolean;
}

export interface General {
  automaticGitRefreshInterval: number;
  periodicRefreshInterval: number;
  revealBehaviour: RevealBehaviour;
  exportPath: string;
  rootFolder: string;
  schemes: string[];
  statusBar: StatusBarMode;
  showIconsInsteadOfTagsInStatusBar: boolean;
  tagGroups: Record<string, string[]>;
  tags: string[];
  showActivityBarBadge: boolean;
}

export interface Highlights {
  customHighlight: Record<string, Attributes>;
  defaultHighlight: Attributes;
  enabled: boolean;
  highlightDelay: number;
  useColourScheme: boolean;
  foregroundColourScheme: string[];
  backgroundColourScheme: string[];
}

export interface Filtering {
  excludedWorkspaces: string[];
  excludeGlobs: string[];
  ignoreGitSubmodules: boolean;
  includedWorkspaces: string[];
  includeGlobs: string[];
  includeHiddenFiles: boolean;
  useBuiltInExcludes: UseBuiltInExcludes;
  builtInExcludes: string[];
}

export interface Tree {
  autoRefresh: boolean;
  disableCompactFolders: boolean;
  expanded: boolean;
  filterCaseSensitive: boolean;
  flat: boolean;
  groupedByTag: boolean;
  groupedBySubTag: boolean;
  hideIconsWhenGroupedByTag: boolean;
  hideTreeWhenEmpty: boolean;
  labelFormat: string;
  scanAtStartup: boolean;
  scanMode: ScanMode;
  showBadges: boolean;
  showCountsInTree: boolean;
  showCurrentScanMode: boolean;
  subTagClickUrl: string;
  sortTagsOnlyViewAlphabetically: boolean;
  sort: boolean;
  tagsOnly: boolean;
  tooltipFormat: string;
  trackFile: boolean;
}

export interface RegexSettings {
  regex: string;
  regexCaseSensitive: boolean;
  subTagRegex: string;
  enableMultiLine: boolean;
}

/**
 * View state the user set by clicking view buttons, kept in the client's
 * workspace storage. A set value overrides the matching `tree.*` setting;
 * an absent one leaves the setting in force.
 */
export interface ViewState {
  flat?: boolean;
  tagsOnly?: boolean;
  expanded?: boolean;
  groupedByTag?: boolean;
  groupedBySubTag?: boolean;
  /** The tree filter text; empty means no filter. */
  filter: string;
  /** Temporary include globs from the folder context menu and scopes. */
  includeGlobs: string[];
  /** Temporary exclude globs from the folder and file context menus and scopes. */
  excludeGlobs: string[];
}

/**
 * The full resolved configuration (spec 6.3). The five groups mirror the
 * `clippings.*` settings; the client also sends the client-only keys in them
 * (`tree.buttons`, `filtering.scopes`, `general.statusBarClickBehaviour`),
 * which the server ignores.
 */
export interface Settings {
  general: General;
  highlights: Highlights;
  filtering: Filtering;
  tree: Tree;
  regex: RegexSettings;
  viewState: ViewState;
  /** Keys of `files.exclude` whose value is exactly `true`. */
  filesExclude: string[];
  /** Keys of `search.exclude` whose value is exactly `true`. */
  searchExclude: string[];
  explorerCompactFolders: boolean;
}

export interface InitializationOptions {
  protocolVersion: number;
  settings: Settings;
}

// ---- styles.rs ----

/** A theme colour id or a CSS colour string. */
export type Colour = { theme: string } | { css: string };

/** What the client renders as an icon. */
export type IconDescriptor =
  | { kind: 'codicon'; name: string; colour: string | null }
  | { kind: 'octicon'; name: string; colour: string }
  | { kind: 'todoTree'; filled: boolean; colour: string }
  | { kind: 'check'; colour: string }
  | { kind: 'default' }
  | { kind: 'folder' }
  | { kind: 'file' };

export interface DecorationStyle {
  color: Colour | null;
  backgroundColor: Colour | null;
  overviewRulerColor: Colour | null;
  overviewRulerLane: number | null;
  borderRadius: string;
  fontStyle: string;
  fontWeight: string;
  textDecoration: string;
  isWholeLine: boolean;
  gutterIcon: IconDescriptor | null;
}

// ---- status.rs ----

export interface StatusBar {
  text: string;
  tooltip: string;
  visible: boolean;
}

export interface Badge {
  value: number;
  tooltip: string;
}

// ---- view/render.rs ----

export type NodeCommand =
  | { kind: 'reveal'; uri: string; position: Position }
  | { kind: 'openUrl'; url: string };

export interface ViewNode {
  id: string;
  label: string;
  description: string | null;
  tooltip: string | null;
  icon: IconDescriptor | null;
  hasChildren: boolean;
  defaultExpanded: boolean;
  contextValue: string | null;
  resourceUri: string | null;
  command: NodeCommand | null;
}

// ---- navigate.rs ----

export type Direction = 'next' | 'previous';

// ---- custom messages ----

export interface ActiveEditorParams {
  uri: string | null;
}

export interface ChildrenParams {
  parent: string | null;
}

export interface ChildrenResult {
  nodes: ViewNode[];
}

export interface FindParams {
  uri: string;
  line: number | null;
}

export interface FindResult {
  /** Each path runs from a top-level node down to a matching node. */
  paths: ViewNode[][];
}

export interface NavigateParams {
  uri: string;
  positions: Position[];
  direction: Direction;
}

export interface NavigateResult {
  ranges: Range[] | null;
}

export interface ExportResult {
  path: string;
  content: string;
}

export interface TreeChangedParams {
  /** Parent IDs to refresh; `null` is the root. */
  refresh: (string | null)[];
}

export interface StylesParams {
  generation: number;
  reset: boolean;
  styles: Record<string, DecorationStyle>;
}

export interface DecorationsParams {
  uri: string;
  version: number;
  generation: number;
  ranges: Record<string, Range[]>;
}

export interface StatusParams {
  instance: string;
  scanning: boolean;
  interrupted: boolean;
  needsScan: boolean;
  error: string | null;
  warnings: string[];
  statusBar: StatusBar;
  badge: Badge;
  viewTitle: string;
  hasSubTags: boolean;
  isEmpty: boolean;
}
