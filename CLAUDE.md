# Clippings

## README files

`README.md` at the repository root and `extension/README.md` are the same
document, kept as two copies: GitHub renders the root one, and vsce packs the
extension one into the vsix for the Marketplace and Open VSX. Whenever you
change either file, apply the identical change to the other, and check they
match before committing:

```sh
cmp README.md extension/README.md
```
