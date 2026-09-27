// `general.schemes` as the client uses it (spec 6.1, 7.4, 7.5). Pure. A
// settings file can hold a value of the wrong type, which VS Code passes on;
// the server then starts on its defaults, and so does the client.

/** todo-tree's default, as in the manifest and the server's settings. */
export const DEFAULT_SCHEMES: readonly string[] = ['file', 'ssh', 'untitled', 'vscode-notebook-cell'];

/** The configured schemes, or the defaults when the value is not a list of strings. */
export function schemeList(value: unknown): readonly string[] {
  return Array.isArray(value) && value.every((s) => typeof s === 'string') ? value : DEFAULT_SCHEMES;
}

/** The language client's document selector: one filter per scheme. */
export function documentSelector(value: unknown): { scheme: string }[] {
  return schemeList(value).map((scheme) => ({ scheme }));
}
