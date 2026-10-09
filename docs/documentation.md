<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Documentation website

The documentation workflow builds the repository's Markdown guides and API
references into one GitHub Pages artifact. The intended public address is
[roolrz.github.io/HypeR](https://roolrz.github.io/HypeR/).

## Sources and generated references

- MkDocs publishes project Markdown files in their existing repository paths.
  Navigation is generated from page titles; adding a tracked Markdown guide
  does not require maintaining a second copy or editing a page inventory.
  Task lists render as disabled checkboxes. Line breaks in inline code become
  spaces so copied commands stay on one line; code blocks retain their layout.
  Keep each inline code span on one source line for consistent GitHub rendering,
  especially when a continuation would start with a list marker such as `+`.
- Rustdoc generates kernel, core, HAL and ABI references separately for AArch64,
  RISC-V and x86-64, using their normal QEMU configurations. It also generates
  application, shared application library, installed SDK and ABI references for both HypeR Native targets,
  using the actual patched Rust standard-library target configuration. Only
  HypeR-owned crates are selected, with `--no-deps`; third-party crates and the
  Rust standard library are compiled as needed but receive no generated reference
  pages. The `lib/` implementation and Native delivery crates belong to the app
  workspace; delivery re-exports link to implementation references, and the Rust
  std owner links upstream without inlining standard-library documentation.
  SDK workspace members and the ABI are explicitly selected because they
  are dependencies outside the application workspace. Explicit local reference
  roots keep cross-crate links independent of the documentation build order.
  Executables are also selected explicitly into a separate reference tree:
  Cargo otherwise skips a binary when its library has the same name. Executable
  type links lead back to the matching library/SDK view. Binary selection follows
  Cargo metadata and includes documented targets under each application's `src/`
  without required features; test fixtures and opt-in probes stay excluded.
- Rust descriptions come from standard `///`, `//!` and `#[doc]` comments.
  Private items are included for kernel development. Rustdoc may list the
  signatures of undocumented items, but the pipeline does not invent prose
  for them or enforce a documentation-coverage quota.
- Doxygen publishes standard C documentation comments from the ABI headers,
  C runtime, userspace executable loader and dynamic loader. Ordinary C comments
  are not converted and undocumented functions do not receive generated
  descriptions.
- Markdown source-code links resolve to files at the built Git revision;
  guide-to-guide links remain inside the website. Mermaid fences render using
  the pinned Mermaid browser script. Guides and API references have their own
  search interfaces.

The pipeline excludes private `.agent`, `.agents` and `.codex` files, ignored
build output and symlinked source files. Third-party prose is not imported as
project guidance. Rustdoc links between HypeR crates stay within the site;
external type and trait links use upstream documentation where available.
The portable atomic facade links its standard-library re-exports upstream
instead of inlining their descriptions and relative Rust book links.
Inherited `Iterator::cmp` descriptions contain an unresolved `Ord` section link
in the pinned Rustdoc version; packaging restores that known upstream URL in
HTML anchors while preserving source examples and other page content.
Cross-crate `Source` links with a stale crate-index-relative path are repaired
only when the exact referenced file exists in that view's source tree.
Test-only configurations are not enabled.
Build scripts and other tools remain accessible through repository source links
and their existing guides; they are not given synthetic function descriptions.

## Pull requests and deployment

Every pull request runs **Documentation / build and links**, including changes
outside Markdown files. Rust code, ABI generation and SDK configuration can
break documentation generation too. This job performs the same complete build
as a deployment and packages the resulting site as an Actions artifact.

The checks reject Rustdoc broken intra-document links and invalid HTML/code-block
attributes, Doxygen generation warnings, MkDocs warnings and missing guide
anchors. A final HTML pass validates links and assets from guide and C reference
pages, including their links into Rust references and the `/HypeR/` project prefix.
It also checks file targets of hyperlinks from every Rustdoc page. Rustdoc checks
Rust intra-document links during compilation; its JavaScript-generated anchors
and optional trait scripts are excluded from the generated-HTML check.
The artifact must also fit the GitHub Pages 1 GB site limit and contain no symlinks.
External website availability is not a merge gate. Code examples and Mermaid
diagrams are rendered as documentation rather than executed as application tests.

Pushes to **main**, the default branch, build and deploy automatically. The
deployment job consumes the artifact already checked by the build job; it does
not rebuild or push generated HTML into Git. Pull requests only build artifacts
and receive no Pages deployment permission. Manual workflow runs deploy only
when run on the default branch.

Repository setup, once:

1. In **Settings → Pages → Build and deployment**, choose **GitHub Actions**.
2. If the `github-pages` environment has deployment branch restrictions, allow
   `main` and retain any desired approval policy.
3. To make failures prevent merging, add **Documentation / build and links**
   to the repository's required status checks/ruleset after its first run.

The workflow validates build artifacts before deployment. Repository settings,
deployment approvals and GitHub service availability are still checked by the
deployment job itself.

## Local build

Install the normal project Rust/LLVM/CMake build prerequisites and Doxygen.
On Ubuntu the additional system package is `doxygen`; on macOS use
`brew install doxygen`. The kernel Makefile selects Homebrew LLVM when present,
so RISC-V documentation does not use Apple's limited target compiler.

From the repository root:

```sh
python3 -m venv target/documentation/venv
target/documentation/venv/bin/python -m pip install -r scripts/docs/requirements.txt
target/documentation/venv/bin/python -m unittest discover -s scripts/docs -p 'test_*.py'
target/documentation/venv/bin/python scripts/docs/build.py
python3 -m http.server --directory target/documentation/site 8000
```

Open `http://localhost:8000/`. Generated SDKs, Cargo output and website files
stay under `target/documentation/`. The first build needs access to pinned Rust
and Python dependencies. The built site is `target/documentation/site`.

After a successful complete build, `scripts/docs/build.py --reuse-api` rebuilds
only the guides around the existing API output for local Markdown editing.
Use the same virtual-environment Python executable. CI never uses this shortcut;
run a complete build after changing Rust/C code, comments or build configuration.

Run `make -C kernel doc ARCH=aarch64` for a standalone kernel Rustdoc view.
The installed `hyper-cargo doc` command selects the same Native target, SDK
crates and standard library as `hyper-cargo build`.
