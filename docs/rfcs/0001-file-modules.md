# RFC 0001: File modules and whole-project compilation

Status: **experimental implementation published in v0.2.0-preview.1**.
This RFC records the file-module contract implemented by the released
experimental compiler. It extends the single-file v0.1 baseline; modules were not shipped in
`0.1.0-preview.1`. The source-unit parser, bounded graph, module-aware checking,
CLI filesystem provider, formatter, and multi-source artifact path are present.
The [local gate](../validation-file-modules.md) records workspace, corpus,
artifact, and compatibility checks. Hosted CI remains commit-specific; this
[publication record](../releases/0.2.0-preview.1-publication.md) identifies the
released source and exact-commit evidence.

## Outcome and boundaries

A program may consist of an entry file and explicitly imported UTF-8 `.larp`
files. The compiler resolves one bounded source graph, checks every reachable
unit, and emits one typed IR program and one verified bytecode module. Files
are compile-time namespaces, not runtime objects, dynamic libraries, or text
includes. The VM continues to execute LARP bytecode directly.

This first module system excludes packages, registries, dependency downloads,
wildcard/selective imports, re-exports, inline modules, mutable globals, module
initializers, separate compilation, runtime loading, and import cycles. Import
order never determines runtime initialization because imports execute no code.

## Source syntax and visibility

The [source-unit grammar fragment](../spec/modules.ebnf) extends the baseline
grammar without changing the legacy parser API.

Imports precede every other declaration in their file:

```larp
import "lib/math.larp" as math;

main_character {
    mf math = 1;
    mf answer = math::answer();
    math::Founder founder = new math::Founder(answer);
    yap(founder.score());
}
```

The imported file has declarations and no `main_character`:

```larp
highkey sidequest answer() -> mf { return 42; }

highkey archetype Founder {
    lowkey mf aura;
    init(mf aura) { self.aura = aura; }
    highkey sidequest score() -> mf uses(read) { return self.aura; }
}
```

These examples illustrate the syntax; executable acceptance uses the tests
listed below rather than treating documentation snippets as passed programs.
The entry unit must contain exactly one `main_character`; every imported unit
must contain none. Existing single-file programs retain their entry semantics.
An empty imported file is valid. Imports inside blocks or after declarations
are errors, including imports of otherwise unused files.

`highkey` exports a top-level function, class, role, or concrete extension.
An omitted modifier or explicit `lowkey` makes that declaration module-private.
Export modifiers do not apply to imports or `main_character`. Existing class
member visibility remains independent: exporting a class exposes its checked
constructor and its public members, never its private fields or methods.
Role methods remain public contracts.

Only a directly imported module's public declarations can be qualified. An
import alias is private to the importing unit; `a::b::item`, re-exporting an
alias, and access to a dependency through another module are unsupported.
Each alias must be unique in its file, even when two imports identify the same
module. Distinct aliases may identify that same module without duplicating it.

`::` separates a module alias and one declaration name in expressions and types,
including nested built-in type arguments, function signatures, constructor
calls, role implementation lists, and extension receiver types. `.` continues
to select a value's member. Aliases occupy their own qualifier namespace, so a
local value named `math` and `math::Founder` coexist without shadowing each
other. An alias alone is not a value or type and cannot be passed or captured.

Each file retains separate ordinary type/value namespaces and its own prelude.
Different modules may define identical declaration names. Declarations bind in
their defining module's lexical environment, including closures and lazy bodies;
the caller's aliases never influence their lookup.

## Public interfaces and extensions

An exported declaration's externally visible interface may mention only public
nominal types or built-in types recursively containing such types. Check function
parameters/results, extension receivers, exported class constructors and public
members, implemented roles, role methods, function types, and all nested
container payloads. Private implementation fields and private helper signatures
may use private types. No inferred or qualified path bypasses export privacy.

A public type from a direct dependency may occur in an exported interface.
This does not re-export its name: a caller needs its own direct import to write
an annotation naming that type. Inference may retain its existing nominal
identity. The compiler must reject private nominal types in public interfaces
even if a caller would only use inference.

Dot extension lookup sees extensions declared in the body's lexical module
only; importing a file does not silently expand that set. Existing real-member
priority is unchanged, including access and signature errors. An explicit
cross-module extension call uses `ext::doubled(value)`, with the receiver supplied
as its first argument. Only exported extensions in the directly imported module
participate. Select by the receiver's exact static type; parameter or result
expectations do not convert the receiver or choose an overload. The receiver is
checked without a contextual expected type, as with dot syntax: `1` defaults to
`mf` even if the only candidate extends `unc`. A typed `unc` variable or explicit
conversion supplies an `unc` receiver. Evaluate the receiver exactly once,
followed by the remaining arguments from left to right. Preserve the extension's
checked signature, effects, and lack of class-private ownership. Real members
do not compete with an explicit qualified extension call.

Multiple exported extensions may share a name when their concrete receiver
types differ; duplicate receiver/name pairs in one lexical module remain errors.
An exported ordinary function and exported extension may not share a basename
in one module. Reject that collision as `LARP-M007`, with a related label on the
first exported declaration. Private ordinary functions may share a basename with
public extensions, and private extensions may share one with public ordinary
functions. Ordinary functions remain non-overloaded. Equal receiver/name pairs
in different modules are independent declarations.

An extension reference such as `ext::doubled` is also a first-class function
value. Its function type retains the receiver as the first parameter; this is
not a bound method. A sole exported candidate can be inferred without an expected
type. With multiple receiver candidates, an expected function type must select
exactly one compatible complete signature, including its effect bound; otherwise
report `LARP-M007`. This does not erase effects or make numeric parameters
convertible. Opt-in imported dot extensions require a later RFC.

## Source graph and resolver boundary

The semantic libraries must not read the filesystem. A pure project loader takes
a root source `{ identity, name, text }` and an injected resolver that receives
the importing source identity and decoded import path and returns the same source
shape or an error. Identity is an opaque provider-owned key; name is a diagnostic
label and text is the immutable source snapshot used throughout compilation.
The resolver is responsible for stable identity/name/text responses during one
load. The loader rejects inconsistent responses for an already observed identity.

The root receives `SourceId(0)`. Additional identities receive consecutive IDs
in deterministic breadth-first discovery order, visiting imports in source
order. The same identity is parsed once even through a diamond or different
aliases. Deduplication compares identities, not text or spelling; two distinct
files with identical contents remain distinct nominal declaration owners.
Source maps preserve original bytes and source IDs through diagnostics and IR.

After discovery, reject self-imports and every directed cycle. Diagnostics identify
the closing import and related import sites along the cycle. Reject duplicate
aliases, wrong entry/imported-main counts, and resolution failures independently
of whether imported names are used. All reachable units are parsed and checked;
unused imports do not hide invalid declarations or effect/region violations.

Graph budgets are 64 distinct source units, 4096 import edges, 4 MiB aggregate
UTF-8 source bytes, and 131,072 aggregate source lines. Each source contributes
its newline count plus one, including a final empty line. Duplicate-identity
imports count as edges, not new units, bytes, or lines. Stop before parsing an
over-budget source or resolving an excess edge. A failed load retains original
loaded sources, including at most one additional resolved file that exceeded a
budget, so its diagnostics remain renderable; these limits do not promise zero
allocation for rejected inputs. Existing syntax/type/verifier budgets still
apply. Artifact limits remain authoritative; graph loading alone does not prove
an arbitrarily large AST or bytecode program fits the artifact format.
These are bounded compiler-input contracts, not process memory or sandbox limits.

The CLI filesystem provider uses the canonical entry file's parent as
the project root. Imports resolve relative to the canonical importing file's
parent and must remain inside that canonical root after symlink resolution.
Paths use `/`, end in `.larp`, and contain no absolute prefix, backslash, or NUL.
`..` is allowed only when the resolved file remains inside the root. Canonical
physical identity deduplicates symlink aliases. A future explicit project-root
option needs its own CLI contract. No network, environment search path, or
working-directory fallback participates. The provider also rejects missing or
non-regular files, invalid UTF-8 contents, and non-UTF-8 canonical paths. Its
bounded reader reads at most 4 MiB plus a sentinel byte per file; provider
failures, including this per-file limit, become `LARP-M001`. Aggregate graph
budget failures use `LARP-M005`. These are the module-provider contracts, not
claims about the unchanged legacy source reader's allocation behavior.
The pure injected-resolver API enforces graph validity rather than filesystem
containment. Canonicalization is ordinary path policy, not a hostile-filesystem
race protection guarantee.

## Compiler and artifact integration

Collect module-local headers and allocate all reachable function/class/role IDs
before checking bodies. Resolution uses module-plus-name symbol keys and explicit
alias edges, never AST text concatenation, identifier rewriting, host-language
translation, or `eval`. Recursive functions inside a module continue to work.
The acyclic import restriction simplifies module dependencies without changing
within-module recursion or nominal object graphs.

Module visibility is distinct from lexical class ownership. Lowered field/member
operations preserve the existing class ownership checks. Effects and regional
specialization traverse cross-file direct calls, closures, roles, and extensions
with the same constraints as local calls. Imports grant no constructor, lazy,
callback, host-capability, or scenario exemption. Module-qualified extension keys
in existing metadata let identical receiver/name pairs in distinct modules
coexist while still being independently verified.

Lower the checked graph to the existing IR/bytecode pipeline and mandatory
verifier. Execution has no module resolver or source-name lookup. Embed every
reachable source with its stable ID; the current artifact source table already
supports multiple files. Reuse its budgets and deleted-source diagnostic behavior.
No artifact wire-version bump is required: the existing format represents source
tables and module-qualified declaration metadata under its existing verification
contracts. The current encoder still writes version 2 and reads versions 1 and 2;
historical artifact fixtures must continue to execute.

## Diagnostics and delivery gates

Module diagnostic families are `LARP-M001` resolution/inconsistent source
identity, `M002` cycles, `M003` duplicate aliases, `M004` entry-point policy,
`M005` graph limits, `M006` private types in public interfaces, and `M007`
exported function/extension name collisions or ambiguous extension values.
Ordinary unknown/private qualified names use the existing name diagnostics;
type, access, effect, and region failures retain their established families.
Diagnostics retain exact primary and related spans across
source files in both text and JSON; source labels never substitute for identity.

Implementation boundaries are:

1. Parsed source units with optional entry blocks, import and qualifier ASTs,
   top-level visibility, and a pure injected-resolver graph with deterministic
   IDs, deduplication, cycle detection, limits, and structured diagnostics.
2. Module-aware symbol/type resolution, exported-interface validation, lexical
   extensions, and whole-graph compilation through IR and verified bytecode.
3. CLI filesystem resolution, project source maps/artifact construction, formatter
   handling of imports/qualifiers, diagnostics, and executable project examples.

M7 acceptance requires actual source and loaded-artifact execution, including GC
stress, for cross-file calls, equal-spelled nominal classes, constructor/member
privacy, roles, captured functions, explicit extensions, effects, lazy evaluation,
and scenarios. Negative cases cover public/private type leakage, alias collisions,
missing/extra main blocks, cycles, inconsistent resolver identities, budgets,
path containment, and effect/region bypass attempts through imported call paths.
Assert exact multi-source JSON primary/related spans after deleting source files.
Run the existing single-file corpus unchanged and keep new project fixtures
distinct from passing programs until the compiler and VM execute them.

The current acceptance tests are
[`project_graph.rs`](../../crates/larp-compiler/tests/project_graph.rs),
[`project_compilation.rs`](../../crates/larp-compiler/tests/project_compilation.rs),
the CLI provider tests in
[`project.rs`](../../crates/larp-cli/src/project.rs),
[`projects.rs`](../../crates/larp-cli/tests/projects.rs), and formatter
[`units.rs`](../../crates/larp-format/tests/units.rs). They exercise graph
identity/budgets, actual compiler-to-VM project execution in normal and stress
modes, imported privacy/effects/regions, source and artifact execution, retained
multi-source diagnostics, path policy, and lossless library formatting.
The [validation record](../validation-file-modules.md) reports executed local
checks and their limits. Hosted checks remain specific to each tested commit.

## Engineering references

The [Rust module reference](https://doc.rust-lang.org/reference/items/modules.html)
provides a useful comparison for file-backed declaration containers, while its
[privacy reference](https://doc.rust-lang.org/reference/visibility-and-privacy.html)
separates item visibility from lookup. LARP deliberately keeps a flat file-import
graph rather than adopting Rust's module tree or re-exports. The
[Go import specification](https://go.dev/ref/spec#Import_declarations) is a useful
comparison for explicit import aliases scoped to a file. LARP does not adopt Go
package initialization, package discovery, or capitalization-based exports.
