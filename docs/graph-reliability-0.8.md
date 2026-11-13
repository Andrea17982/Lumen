# Graph reliability — TypeScript and Godot imports

Eighth 0.8 checkpoint, fixed before implementation on 11 November 2026. The published 0.7.1 stays unchanged. Work uses isolated synthetic projects only, no Rodent Heroes assets, new dependencies or model inference. The existing live ledger remains 13 spent slots.

## Acceptance corpus

Exercise the public graph/impact path, not just a resolver helper: TypeScript `.mjs/.cjs` imports of `.mts/.cts` sources, extension substitution in the presence of emitted JavaScript, native JavaScript with a TypeScript neighbor, and missing/out-of-repository targets. For Godot, cover quoted `res://` paths, relative script inheritance, project boundaries and a neighboring C# type reference. Use aliased short names where possible to avoid heuristic symbol matching hiding a missing import edge.

Require the expected direct dependency and reverse impact, absence of the named false dependents, correct declaration locations, and fresh results after edited imports, rename and deletion. Existing before/after impact snapshots must retain callers of deleted targets. Persisted indexes must be invalidated when extraction semantics change. Keep an unchanged-refresh control.

Primary outcome: all declared positive/negative corpus expectations pass; record baseline failures before fixes. TypeScript's official extension-substitution rules and Godot's script/resource semantics guide the expected targets, with local installed-compiler/engine checks where useful. The graph remains approximate: dynamic paths, arbitrary aliases/configuration and full language-semantic references are not certified. A parser migration requires separate demonstrated benefit.

## Results and implementation

The original core corpus has **23 expectations**: eleven direct-import/reverse-impact pairs and one C#→GDScript reference control. The 0.7.1 baseline passes **9/23** and fails **14/23**. The candidate passes **23/23**. These are fixed fixture expectations, not an estimate of whole-project accuracy. [Aggregate evidence](data/graph-reliability-0.8-results.json).

- Imports from typed `.ts/.tsx/.mts/.cts` files now apply the scoped TypeScript extension substitutions for `.js/.jsx/.mjs/.cjs`, including declaration-file fallbacks and `.js`→`.jsx` when appropriate. TypeScript source wins over an emitted JavaScript neighbor. Native JavaScript explicit imports retain their runtime target and do not invent a missing typed counterpart. Existing extensionless/alias heuristics remain approximate; this is not a complete tsconfig/bundler resolver.
- GDScript literal `res://` loads/preloads accept either quote style; script-relative quoted `extends` resolves inside the governing Godot project. Resource `.`/`..` segments normalize without inventing edges above the project/repository boundary. Absolute paths, unsupported schemes and escaping targets stay unresolved. Relative `load`/`preload` and UID mapping are not added by this change.
- Comment-only lines in TS/JS, C# and GDScript no longer create declarations, explicit imports or identifier-reference edges. This is intentionally limited to full-line comments; inline/block comments, multiline strings and dynamic expressions still require a richer parser for complete handling.
- The persistent index format advances **3→4**, forcing re-extraction of earlier records even when file metadata matches. An unchanged refresh still reuses the index/graph. Rename/edit/deletion invalidation and immutable pre-edit snapshots are verified. The handoff retains newly recognized callers/tests after their target files are deleted.

The [TypeScript 4.7 module documentation](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-4-7) explains `.mts/.cts` emission; the [Godot GDScript reference](https://docs.godotengine.org/en/4.4/tutorials/scripting/gdscript/gdscript_basics.html) covers quoted paths and inheritance. Independent local checks use **TypeScript 5.9.3**, **Node 24.2.0** and **Godot 4.7.2 Mono**: four compiler-resolution checks, native JavaScript resolution/missing-module checks, and actual single-quoted preload/script-relative inheritance execution. [Oracle record](data/graph-import-oracles-0.8-results.json).

Windows checks pass: **163 Rust tests**, 14 opt-in probes ignored; Clippy with warnings denied; debug build; formatting/documentation checks. Five native graph MCP checks validate callers/tests, declaration line/body, edits/renames within one running process, disk-index reload after restart and deleted targets. No GUI/provider/account inference or universal C#/Godot semantic certification follows from these native graph checks.

## Reproduction and limits

`cargo test --manifest-path src-tauri/Cargo.toml --lib` includes the corpus, comment/resource controls, mutation/cache migration and deleted-target handoff checks. `node tests/graph-import-oracles.mjs` uses installed TypeScript/Node and the existing Godot console executable (override with `LUMEN_GODOT_EXE`); it generates only an isolated temporary project and no assets/model calls. `node tests/native-graph.mjs` runs the built debug executable's actual stdio graph MCP against separate temporary data/project folders. `node tests/graph-reliability-report.mjs` aggregates the retained local baseline/test/native records into sanitized JSON. Raw records remain ignored under `.impeccable/`.

There is no new parser dependency, index-speed/token-saving claim or evidence that the model itself produces better code. Size/mtime still drive ordinary refresh; content edits preserving both stamps and configuration-only resolution changes are not certified. Arbitrary aliases, package exports, generated code, dynamic resource paths, complete language semantics and non-Windows desktop acceptance remain open. The published 0.7.1 and its tag are unchanged; this checkpoint belongs to the next 0.8 implementation branch.
