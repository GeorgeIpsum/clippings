// Types for the parts of @primer/octicons the extension uses; the package
// ships no declarations.
declare module '@primer/octicons' {
  interface Octicon {
    readonly symbol: string;
    toSVG(options?: Record<string, string | number>): string;
  }
  const octicons: Readonly<Record<string, Octicon>>;
  export = octicons;
}
