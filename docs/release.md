# Releasing Clippings

How the ten packages are built, how to cut a release, how to turn on publishing, and how to check a release before and after. The rules come from spec section 8.

## What a release contains

| Asset | What it is |
|---|---|
| `clippings-<target>-<version>.vsix` | one package per VS Code target, with that target's server in `extension/bin/` beside `platform.ok` |
| `clippings-universal-<version>.vsix` | the package without a server; it works only when `clippings.server.path` points at a server the user built |
| `clippings-darwin-<arch>.dSYM.zip` | macOS debug symbols |
| `clippings-win32-<arch>.pdb` | Windows debug symbols |
| `clippings-<linux or alpine>-<arch>.debug` | Linux and Alpine debug symbols; the shipped binary's `.gnu_debuglink` names this file |

The targets are `win32-x64`, `win32-arm64`, `linux-x64`, `linux-arm64`, `linux-armhf`, `alpine-x64`, `alpine-arm64`, `darwin-x64` and `darwin-arm64` — nine targets, ten packages (the nine plus universal), nine symbol files (one per target; the universal package has none). The `linux-*` servers need glibc 2.28 or later.

## Versions

Versions are plain `major.minor.patch`: the Marketplace rejects semver pre-release suffixes. An **odd minor** version is a pre-release, packaged and published with `--pre-release`; an **even minor** version is stable. So `0.1.x` is the first pre-release line and `0.2.0` the first stable release.

The version lives in two places that must agree: `extension/package.json` and `[workspace.package]` in the root `Cargo.toml`. `node extension/scripts/version.mjs` checks both and prints the channel; `.github/workflows/release.yml`'s `plan` job runs the same script with `--tag <tag>` to also check that the tag is `v` plus that version.

## Toolchain pins

**Rust** is pinned to the same version in four places, kept in sync by hand:

- `rust-toolchain.toml` (`channel = "1.98.1"`) — documents the pin for anyone building locally with rustup; GitHub Actions does not read this file.
- `dtolnay/rust-toolchain@1.98.1` in `.github/workflows/ci.yml`'s `rust` job.
- `dtolnay/rust-toolchain@1.98.1` in `.github/workflows/ci.yml`'s `extension` job.
- `dtolnay/rust-toolchain@1.98.1` in `.github/workflows/build.yml`'s `server` job.

To bump it, edit all four in the same commit (the `channel` line and the three `dtolnay/rust-toolchain@<version>` lines), then let CI prove it: `cargo fmt`, `cargo clippy` and `cargo test` all run against whichever version is pinned.

**Node** is pinned to `22.23.3` through `actions/setup-node@v5`'s `node-version:` input, in five places: `ci.yml`'s `extension` job, `build.yml`'s `package` job, and `release.yml`'s `plan`, `marketplace` and `open-vsx` jobs. There is no `.nvmrc` and no `engines.node`; `extension/package.json`'s `engines` field only pins the VS Code API version (`^1.91.0`). To bump Node, edit all five `node-version:` lines to match.

**Other pins**, each in one place:

- `cargo-zigbuild==0.23.4` and `ziglang==0.15.2`, installed with pip in `.github/workflows/build.yml`'s `server` job;
- the `debian:10` and `alpine:3.22` probe images, pinned by digest in `scripts/dist/check.sh`, with the tag and date they were taken from in a comment beside each;
- `ovsx@1.2.0`, run with `npx` in `release.yml`'s `open-vsx` job (`@vscode/vsce` is a dev dependency, pinned by `extension/package.json` and `pnpm-lock.yaml`);
- VS Code `1.91.0`, the `extension` job's matrix in `ci.yml`, which must equal the `engines.vscode` floor in `extension/package.json`;
- actionlint `1.7.12`, in `ci.yml`'s `lint` job (both the download script's tag and its version argument). shellcheck is whatever the `ubuntu-latest` image ships.

## Cutting a release

1. Bump the version in `extension/package.json` and `Cargo.toml`'s `[workspace.package]` `version`, run `cargo check` so `Cargo.lock` follows, and commit on `main`.
2. Check it locally:

   ```sh
   node extension/scripts/version.mjs --tag v0.1.0
   ```

3. Wait for CI (`.github/workflows/ci.yml`, which runs on every push to `main`) to pass on that commit: `gh run list --workflow ci.yml --branch main --limit 1`.
4. Run the release workflow as a dry run and wait for it to pass (see [Dry run](#dry-run)); a dry run needs no tag, so run it from `main` before tagging.
5. Tag that commit and push the tag:

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

The tag starts `.github/workflows/release.yml`, whose jobs run in this order:

1. **plan**: refuses the tag unless its commit is on `main` and CI passed on it (below), checks the tag against the version, and decides the channel and which publishing jobs will run, from the `MARKETPLACE_AUTH` and `OPEN_VSX_PUBLISH` repository variables;
2. **build** (`.github/workflows/build.yml`): builds the server for the nine targets, signs the macOS and Windows binaries when configured, checks each, and packages the ten VSIX files;
3. **release**: creates the GitHub release for the tag with generated notes, marked pre-release for odd minors, with the ten packages and nine symbol files attached;
4. **marketplace** and **open-vsx**: publish the ten packages, each only when its repository variable turns it on (see [Publishing](#publishing)).

`build` builds, checks and packages but runs no tests, so a release is only as tested as CI has made its commit. On a tag, `plan` therefore refuses to go on, and nothing is built, released or published, when:

- the tagged commit is not an ancestor of `origin/main` (a tag on a feature branch, or on a commit that never reached `main`); or
- the commit's `rust` and `extension` check runs from `ci.yml` are missing, still running, or did not all conclude `success`. `plan` needs at least one of each, so a commit CI never ran on is refused too.

To release after a refusal, fix the cause (merge to `main`, or wait for or fix CI), then either re-run the release run's jobs (if the commit was right and CI was just not finished yet) or delete the tag (`git tag -d v0.1.0 && git push origin :refs/tags/v0.1.0`) and tag the right commit. A manual run from a branch, including a dry run, skips this check.

If a job fails partway, see [Recovering a failed release](#recovering-a-failed-release). Publishing uses `--skip-duplicate`, so re-running a publish job skips the packages that were already accepted.

## Recovering a failed release

If `build` (or a job it calls) fails, just re-run the failed jobs from the workflow run page, or `gh run rerun <run-id> --failed`; nothing has been created yet, so there is nothing to clean up first.

If the `release` job fails, check whether it got as far as its "Create the GitHub release" step: `gh release create` creates the release object for the tag and then uploads the assets, so a failure partway through (an asset upload dropping out, a timeout) can leave the release behind, already existing. Re-running the `release` job as-is then fails again, because `gh release create` refuses to create a release that already exists for the tag. Recovery:

```sh
gh release delete v0.1.0 --yes
```

This deletes only the GitHub release, not the `v0.1.0` git tag (`--cleanup-tag` would also delete the tag — don't pass it; the tag is what the workflow re-runs from). Then re-run the failed job, or the whole run, from the Actions run page, or `gh run rerun <run-id> --failed`; `build`'s artifacts are still attached to that run, so nothing rebuilds.

`marketplace` and `open-vsx` publish with `--skip-duplicate`, so if one of them fails after partially publishing, re-running it only sends the packages that were not already accepted.

## Dry run

Actions > Release > Run workflow, with **dry-run** ticked (the default), or:

```sh
gh workflow run release.yml --ref main
gh run watch
```

Being explicit about the input also works:

```sh
gh workflow run release.yml --ref <branch> -f dry-run=true
gh run watch
```

A dry run skips only the **release**, **marketplace** and **open-vsx** jobs (they show as skipped in the run). **plan** and **build** run exactly as they would for a real release — including `build.yml`'s `sign-macos` and `sign-windows` jobs, when those are configured. Signing is gated on its own repository variables (`MACOS_SIGNING_IDENTITY`, `WINDOWS_SIGNING_ENDPOINT`), independent of dry-run: a dry run with signing configured still signs the binaries. Today, with no signing configured in this repository, `sign-macos` and `sign-windows` are skipped in every run, dry or not — a dry run happens to show them as skipped, but not because it's a dry run.

The packages are the run's `vsix` artifact, and each target's server and symbols are the `server-<target>` and `symbols-<target>` artifacts.

A manual run with dry-run off is refused unless it runs from a `v*` tag.

## Where packages come from

Only two workflows ever run `build.yml`'s server matrix and packaging job:

- **`.github/workflows/dist.yml`**, on every push to `main`. This used to be a `dist` job inside `ci.yml`; it moved into its own push-only workflow because a `workflow_call` job that asks for `id-token: write` (needed for macOS and Windows signing) inside a workflow that also triggers on `pull_request` can have its *entire run* rejected at load time by GitHub's permission cap on fork and Dependabot PRs — even on a run where the gated job would be skipped. A push-only workflow sidesteps that.
- **`.github/workflows/release.yml`**'s `build` job, from a `v*` tag or a manual (dry or real) run.

`.github/workflows/ci.yml` never builds the packages, and its top-level `permissions: contents: read` requests nothing elevated: `dist.yml` and `release.yml`'s `build` job both need `id-token: write` for the optional signing jobs, and granting that to a workflow with a `pull_request` trigger is exactly what makes GitHub reject fork PRs at load time. Keeping `ci.yml` minimal everywhere is what lets fork PRs run it at all.

## Publishing

Publishing is off until the extension has a real publisher. Today `publisher` in `extension/package.json` is the placeholder `clippings-dev` (spec 7.1), no publishing variables are set, and both publishing jobs are skipped. Each store is turned on separately, by a **repository variable** (Settings > Secrets and variables > Actions > Variables, at repository scope):

| Variable | Value | Effect on a release |
|---|---|---|
| `MARKETPLACE_AUTH` | `entra` (preferred) or `pat`; unset skips the Marketplace | the `marketplace` job runs, signing in with Microsoft Entra ID or with the `VSCE_PAT` secret |
| `OPEN_VSX_PUBLISH` | `true`; unset skips Open VSX | the `open-vsx` job runs, with the `OVSX_PAT` secret |

`release.yml`'s `plan` job reads these to decide which publishing jobs run. `plan` has no `environment:`, so it can only see repository-scoped variables; that is why the switches are repository variables. `plan` refuses any other value, and it checks them on every run, dry runs included, so a dry run catches a typo. A dry run never publishes, whatever they are set to.

The tokens themselves are **environment secrets**, not repository secrets: `VSCE_PAT` on the `marketplace` environment and `OVSX_PAT` on the `open-vsx` environment (Settings > Environments > `<name>` > Environment secrets). Only a job that names the environment and satisfies its deployment rule can read them (see [Environment protection](#environment-protection)). If a job's variable turns it on but its environment has no token, the job fails with an error instead of skipping.

### Switching the publisher ID

1. Create a Marketplace publisher at <https://marketplace.visualstudio.com/manage>. Its ID is permanent and becomes the first half of the extension ID, `<publisher>.clippings`.
2. Replace `clippings-dev` with the new ID in:
   - `extension/package.json` (`publisher`);
   - `extension/src/test/unit/manifest.test.ts`;
   - the `getExtension('clippings-dev.clippings')` calls in `extension/src/test/integration/` (`helpers.ts`, `activation.test.ts`, `commands.test.ts`, `server.test.ts`);
   - the comment on the `marketplace` job in `.github/workflows/release.yml`, and the log path under [Checking packages](#checking-packages).

   `git grep --untracked -l clippings-dev -- extension .github docs/release.md` lists them. `extension/scripts/vsix.test.mjs` uses the ID only as sample fixture data and can stay.
3. Update spec 7.1, which records the placeholder.

Use the same ID as the Open VSX namespace so the extension has one ID everywhere.

### Visual Studio Marketplace: Microsoft Entra ID (preferred)

Azure DevOps global personal access tokens, the kind `vsce` has used, are retired on 1 December 2026. The `marketplace` job therefore signs in with Microsoft Entra ID through GitHub's OIDC token (`azure/login`) and publishes with `vsce publish --azure-credential`. No secret is stored.

One-time setup:

1. In the Azure portal, create a **user-assigned managed identity** (Managed Identities > Create; any subscription and resource group), or an **app registration** in Microsoft Entra ID. Note its **client ID** and your **tenant ID**.
2. Add a **federated credential** to it (managed identity: Settings > Federated credentials; app registration: Certificates & secrets > Federated credentials), with the scenario **GitHub Actions deploying Azure resources**:
   - organization `GeorgeIpsum`, repository `clippings`;
   - entity type **Environment**, environment name `marketplace`. The release workflow's `marketplace` job runs in that environment, so the subject is `repo:GeorgeIpsum/clippings:environment:marketplace`;
   - audience `api://AzureADTokenExchange`, the default.
3. Add the identity to the Marketplace publisher: at <https://marketplace.visualstudio.com/manage/publishers/>, open the publisher's **Members**, add the identity by its name (for an app registration, the name of the service principal) and give it the **Contributor** role. Publisher membership needs the identity to be known to Azure DevOps: if the Members dialog cannot find it, first add it as a user of an Azure DevOps organization in the same tenant (Organization settings > Users, Stakeholder access is enough).
4. In the GitHub repository, set the **repository variables** (Settings > Secrets and variables > Actions > Variables, at repository scope, not on the `marketplace` environment: `plan` checks them) `AZURE_CLIENT_ID` and `AZURE_TENANT_ID`, and set `MARKETPLACE_AUTH` to `entra`. `plan` fails a run with `MARKETPLACE_AUTH=entra` unless both Azure variables are set.
5. Protect the `marketplace` environment (see [Environment protection](#environment-protection)) so publishing can't be triggered from an arbitrary branch.

A subscription is not needed: the login uses `allow-no-subscriptions`.

### Visual Studio Marketplace: personal access token (fallback)

With `MARKETPLACE_AUTH` set to `pat`, the job publishes with a `VSCE_PAT` secret on the `marketplace` environment: an Azure DevOps personal access token with the **Marketplace > Manage** scope. Marketplace publishing has traditionally needed a global (*All accessible organizations*) token, and global tokens are retired on 1 December 2026, so treat this path as temporary and prefer `entra`. With `MARKETPLACE_AUTH` unset, the `marketplace` job is skipped.

### Open VSX

1. Create an Eclipse Foundation account at <https://accounts.eclipse.org> and sign in to <https://open-vsx.org> with GitHub, linking the Eclipse account in your profile settings.
2. Sign the **Eclipse Foundation Open VSX Publisher Agreement** from your Open VSX profile.
3. Create an access token under Settings > Access Tokens.
4. Create the namespace, which must equal the `publisher` ID, once:

   ```sh
   npx ovsx create-namespace <publisher> -p <token>
   ```

5. Create the `open-vsx` environment (Settings > Environments > New environment), give it the `v*` tag rule (see [Environment protection](#environment-protection)), and save the token there as the environment secret `OVSX_PAT`.
6. Set the repository variable `OPEN_VSX_PUBLISH` to `true`.

The `open-vsx` job runs in the `open-vsx` environment and publishes each package with `ovsx publish --no-dependencies --skip-duplicate` (pinned to `ovsx@1.2.0` in the workflow), plus `--pre-release` on odd minors. It is skipped unless `OPEN_VSX_PUBLISH` is `true`.

## Environment protection

`marketplace`, `open-vsx` and `signing` gate OIDC sign-in and secrets, not code review: any workflow file that adds `environment: marketplace`, `environment: open-vsx` or `environment: signing` to a job gets that environment's secrets, variables and federated-credential subject, on whatever branch or tag that workflow runs from. With no deployment rule configured, that is every branch — including a feature branch's edited copy of a workflow file. Configure a rule for all three environments:

1. Repository Settings > Environments > select (or create) the environment, named exactly `marketplace`, `open-vsx` or `signing`.
2. Under **Deployment branches and tags**, change *No restriction* to *Selected branches and tags*, then **Add deployment branch or tag rule**.
3. For `marketplace` and `open-vsx`: add a **Tag** rule, pattern `v*`. The publishing jobs only ever run as part of a real release (a pushed `v*` tag, or a manual dry-run-off run that `plan` already refuses unless it is on a `v*` tag), so a tag-only rule is exact. The rule is what keeps `VSCE_PAT` and `OVSX_PAT` away from every branch.
4. For `signing`: add a **Tag** rule `v*` *and* a **Branch** rule `main`. `dist.yml`'s `build` job also signs on every ordinary push to `main` (see [Where packages come from](#where-packages-come-from)), so restricting `signing` to tags alone would silently stop main-branch signing.
5. Optionally, under **Deployment protection rules**, tick **Required reviewers** and add reviewers, on any of the environments. This holds every run — regardless of branch or tag — for a manual approval, and can be used instead of, or together with, the branch/tag rule above.
6. Add the secrets on each environment's page (Settings > Environments > `<name>` > Environment secrets): `VSCE_PAT` on `marketplace`, `OVSX_PAT` on `open-vsx`, and the certificate and notary secrets on `signing`. The values that decide whether a job runs (`MARKETPLACE_AUTH`, `OPEN_VSX_PUBLISH`, `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `MACOS_SIGNING_IDENTITY`, `WINDOWS_SIGNING_ENDPOINT`) stay repository variables, because `plan` or a job-level `if:` reads them before any job enters its environment; see [Publishing](#publishing) and [Signing](#signing).

## Signing

Signing is optional and off by default. `build.yml`'s `sign-macos` and `sign-windows` are separate jobs, not steps folded into the ordinary `server` matrix job — each declares `environment: signing` and a job-level `if:` gate (`vars.MACOS_SIGNING_IDENTITY != ''` and `vars.WINDOWS_SIGNING_ENDPOINT != ''` respectively). GitHub evaluates a job's `if:` before the job enters its environment, so each gate variable must be a **repository**-scoped variable, not one scoped to the `signing` environment — an environment-scoped value there is invisible to the gate and the job never runs. Unconfigured, both jobs simply show as **skipped** in the run; the ordinary `server` job just builds the binary with whatever signature the platform linker applies by default (an ad-hoc one on macOS, none on Windows).

Packages installed from the Marketplace, Open VSX or a VSIX file do not need signed binaries: files that VS Code extracts carry no macOS quarantine attribute and no Windows Mark-of-the-Web, so Gatekeeper and SmartScreen do not inspect the server. Signing matters for binaries users download from the GitHub release and run themselves, and for managed machines whose policies require signed code.

### macOS: Developer ID and notarization

`sign-macos` runs after `server`, downloads both `server-darwin-*` artifacts, restores their execute bit (`upload-artifact`/`download-artifact` don't preserve it), and runs `scripts/dist/sign-macos.sh` on each binary: it imports the certificate into a throwaway keychain, signs the binary with the hardened runtime and a secure timestamp, verifies the signature, and submits a zip of the binary to `notarytool`, failing unless Apple accepts it. A bare binary cannot have its ticket stapled; Gatekeeper finds the ticket online. The dSYM is unaffected, because signing does not change the binary's UUID. The job then re-checks both signed binaries with `scripts/dist/check.sh` and re-uploads them, overwriting the unsigned `server-darwin-x64`/`server-darwin-arm64` artifacts that `package` later downloads.

Configure, in the repository settings:

| Name | Kind | Value |
|---|---|---|
| `MACOS_SIGNING_IDENTITY` | repository variable (must not be `signing`-environment-scoped — `sign-macos`'s job-level `if:` reads it before the job enters that environment) | the certificate's name, `Developer ID Application: <Name> (<TEAMID>)` |
| `MACOS_NOTARY_TEAM_ID` | repository variable, or `signing`-environment variable (only read inside the job's own steps, so environment scope works too) | the Apple Developer team ID |
| `MACOS_CERTIFICATE_P12` | `signing`-environment secret (recommended); repository secret is the fallback | base64 of a `.p12` export of the **Developer ID Application** certificate and its private key (`base64 -i cert.p12`) |
| `MACOS_CERTIFICATE_PASSWORD` | `signing`-environment secret (recommended); repository secret is the fallback | the `.p12` password |
| `MACOS_NOTARY_APPLE_ID` | `signing`-environment secret (recommended); repository secret is the fallback | the Apple ID that notarizes |
| `MACOS_NOTARY_PASSWORD` | `signing`-environment secret (recommended); repository secret is the fallback | an app-specific password for that Apple ID (appleid.apple.com > Sign-In and Security) |

`build.yml` declares the four certificate/notary secrets as optional `workflow_call` inputs, and `dist.yml` and `release.yml` forward their own repository secrets of the same name into it — but that's only the fallback path the build workflow's own comments describe. The intended setup is simpler: add the four secrets directly on the `signing` environment (Settings > Environments > `signing` > Environment secrets). A job that declares `environment: signing`, as `sign-macos` does, reads that environment's secrets directly, with no forwarding needed, and they never come into scope for the ordinary `server` job or its third-party actions.

Signing runs when `MACOS_SIGNING_IDENTITY` is set, and the check that follows probes the signed binary.

### Windows: Azure Artifact Signing

`sign-windows` runs after `server`, downloads both `server-win32-*` artifacts, signs both with Azure Artifact Signing (formerly Trusted Signing) through `azure/artifact-signing-action@v2`, then re-checks the signed `win32-x64` binary with `scripts/dist/check.sh` (`win32-arm64` is checked `--no-probe` here, since this job runs on the `windows-latest` x64 runner) and re-uploads both artifacts, overwriting the unsigned ones. `probe-win32-arm64` then probes the signed Arm64 binary for real on a Windows-on-Arm runner before `package` uses it. The PDBs still match the signed binaries, because an Authenticode signature does not change the debug directory.

One-time setup:

1. Create an **Artifact Signing account** and complete **identity validation**, then create a **certificate profile** (Public Trust) in it.
2. Use the same kind of Azure identity as for the Marketplace (it can be the same one) and give it the **Artifact Signing Certificate Profile Signer** role on the signing account.
3. Add a federated credential for entity type **Environment**, environment name `signing` (subject `repo:GeorgeIpsum/clippings:environment:signing`). Both `sign-macos` and `sign-windows` run in that environment. `dist.yml`'s `build` job also signs on every push to `main`, so the `signing` environment's deployment rule needs to allow `main` as well as `v*` tags — see [Environment protection](#environment-protection).
4. Set the repository variables `AZURE_CLIENT_ID` and `AZURE_TENANT_ID` — shared with the Marketplace, and kept at repository scope there because the `plan` job checks them; reusing that same repository-level pair here is simplest — and:

| Variable | Value |
|---|---|
| `WINDOWS_SIGNING_ENDPOINT` | the account's region endpoint, such as `https://eus.codesigning.azure.net/` — must be a repository variable: `sign-windows`'s job-level `if:` reads it before the job enters the `signing` environment, and a job-level `if:` cannot see an environment-scoped variable |
| `WINDOWS_SIGNING_ACCOUNT` | the Artifact Signing account name |
| `WINDOWS_SIGNING_PROFILE` | the certificate profile name |

The job runs when `WINDOWS_SIGNING_ENDPOINT` is set. Signatures are timestamped by `http://timestamp.acs.microsoft.com`, so they outlive the short-lived signing certificates.

## How the build works

`scripts/dist/build.sh <target>` builds one server with the `dist` profile: the `release` profile (fat LTO, one codegen unit, `panic = "unwind"`) plus full debug info, which it then moves into the symbols file:

- **macOS** builds natively with `split-debuginfo = "packed"`; rustc writes the dSYM, then strips the debug info from the binary (`strip = "debuginfo"`).
- **Windows** builds natively; MSVC writes the PDB beside the binary, which carries no debug info.
- **Linux and Alpine** build with `cargo zigbuild`. Linux (glibc) targets use `<triple>.2.28` — zigbuild's glibc-suffixed target — for the 2.28 floor; Alpine (musl) targets have no glibc floor to pin, so `scripts/dist/target.sh` leaves `glibc` empty for them and `build.sh` builds the plain musl triple with no suffix. `llvm-objcopy` from the rustup `llvm-tools` component then splits the debug info into the `.debug` file and strips it from the binary (`--strip-debug`) for both. `CLIPPINGS_BUILD_TOOL=cargo` builds with plain cargo instead, for the `debian:10` container fallback the spec allows; zigbuild builds all five Linux and Alpine targets today, so CI does not use it.

macOS, Linux and Alpine binaries keep their symbol table, so a panic backtrace in the Clippings output channel (the client sets `RUST_BACKTRACE=1`) names its functions; file and line numbers need the symbols file. This makes those binaries larger: 14 % for `darwin-arm64` measured locally, and 15 to 27 % across targets in the prototype builds. Windows binaries cannot do this: MSVC executables carry no symbol table, and names come only from the PDB, which stays a release asset, so Windows backtraces show `<unknown>` frames unless the PDB is placed beside `clippings.exe`.

The server uses mimalloc on Windows and the system allocator elsewhere.

`scripts/dist/check.sh <target> <binary>` then checks it:

- a macOS, Linux or Alpine binary must have a `clippings::main` symbol, and a Linux or Alpine binary no `.debug_info` section;
- a glibc binary's highest `GLIBC_` symbol version, from `readelf -V`, must not exceed 2.28;
- `clippings probe` must print JSON whose `protocolVersion` equals `PROTOCOL_VERSION` in `extension/src/protocol.ts`. Linux binaries run in `debian:10` (glibc 2.28 exactly) and Alpine binaries in `alpine:3.22`, the Arm ones under qemu; macOS binaries run on the Apple silicon runner, `darwin-x64` under Rosetta; the `win32-arm64` binary runs on a Windows Arm runner (`probe-win32-arm64`), since the x64 Windows runner that builds it cannot execute an Arm64 binary.

`build.yml`'s `package` job bundles the extension once with `pnpm -C extension build --production`, then runs `extension/scripts/package.mjs` for each target and for the universal package. It copies the server and `platform.ok` into `extension/bin`, runs `vsce package --no-dependencies --target <target>` (plus `--pre-release` for odd minors), and checks the result with `extension/scripts/vsix.mjs`: the expected files and nothing else, the server's format and architecture, its executable bit, the manifest's target, version and pre-release flag, and no server in the universal package.

## Checking packages

Check a downloaded package:

```sh
node extension/scripts/vsix.mjs clippings-linux-x64-0.1.0.vsix --target linux-x64
node extension/scripts/vsix.mjs clippings-universal-0.1.0.vsix --universal
```

Try one in an isolated VS Code, without touching your own profile (keep the directory short: VS Code's IPC socket path has a length limit on macOS):

```sh
dir=$(mktemp -d /tmp/cv.XXXX)
code --user-data-dir "$dir" --extensions-dir "$dir/ext" --install-extension clippings-darwin-arm64-0.1.0.vsix
code --user-data-dir "$dir" --extensions-dir "$dir/ext" --new-window some-folder
grep -r "Using bundled server" "$dir/logs"
```

The Clippings output channel, also written to `logs/*/window1/exthost/clippings-dev.clippings/Clippings.log`, starts with `Using bundled server (<extension>/bin/clippings): clippings <version> (<arch>-<os>)`.

## Packaging locally

On macOS or Windows, for the machine's own target:

```sh
scripts/dist/build.sh darwin-arm64
pnpm -C extension install
pnpm -C extension build --production
pnpm -C extension package --target darwin-arm64 --server ../dist/darwin-arm64/clippings
pnpm -C extension package --universal
```

Packages land in `dist/vsix/`. Linux targets need `cargo-zigbuild` and zig, or run the same commands in a Linux container.
