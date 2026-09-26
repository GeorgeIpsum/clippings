// Typed JSON-RPC message types for the custom `clippings/*` messages.

import { NotificationType, RequestType } from 'vscode-languageclient/node';
import {
  Method,
  type ActiveEditorParams,
  type ChildrenParams,
  type ChildrenResult,
  type DecorationsParams,
  type ExportResult,
  type FindParams,
  type FindResult,
  type NavigateParams,
  type NavigateResult,
  type Settings,
  type StatusParams,
  type StylesParams,
  type TreeChangedParams,
} from '../protocol';

export const Configure = new NotificationType<Settings>(Method.configure);
export const ActiveEditor = new NotificationType<ActiveEditorParams>(Method.activeEditor);
export const Rescan = new NotificationType<Record<string, never>>(Method.rescan);
export const StopScan = new NotificationType<Record<string, never>>(Method.stopScan);
export const Children = new RequestType<ChildrenParams, ChildrenResult, void>(Method.children);
export const Find = new RequestType<FindParams, FindResult, void>(Method.find);
export const Navigate = new RequestType<NavigateParams, NavigateResult, void>(Method.navigate);
export const Export = new RequestType<Record<string, never>, ExportResult, void>(Method.export);
export const TreeChanged = new NotificationType<TreeChangedParams>(Method.treeChanged);
export const Styles = new NotificationType<StylesParams>(Method.styles);
export const Decorations = new NotificationType<DecorationsParams>(Method.decorations);
export const Status = new NotificationType<StatusParams>(Method.status);
