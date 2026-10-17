# Compiler and runtime architecture

Status: scalar, closure, option/result, and collection programs compile through
typed IR and verified in-memory bytecode to VM execution. Array/closure tracing
is implemented; objects, scenario regions, and persisted artifacts remain planned.
The pipeline below includes both current contracts and their intended extensions;
see [implementation status](implementation-status.md) for the supported subset.

## Pipeline and contracts

```mermaid
flowchart LR
    Source[UTF-8 source] --> Lex[Lexer with spans]
    Lex --> Parse[Recursive descent + Pratt parser]
    Parse --> AST[AST]
    AST --> Names[Scopes and function symbols]
    Names --> Types[Types and finite effects]
    Types --> IR[Typed control-flow IR]
    IR --> Lower[Bytecode lowering]
    Lower --> Verify[Bytecode verifier]
    Verify --> VM[LARP stack VM]
    VM --> Host[Observation sink]
```

1. **Source:** assign file identities; retain original bytes, line starts, and
   validated half-open spans. Diagnostics refer to file IDs, not copied lines.
2. **Lexer/parser:** handwritten scanner and recursive-descent declarations with
   a Pratt expression parser. This makes unusual token pairs and expression
   precedence explicit. Recover at `;` and balanced braces; preserve error nodes
   so one typo does not produce dozens of false errors. AST nodes retain spans.
3. **Resolution:** collect declaration headers first, then bind bodies to stable
   symbol IDs and lexical scopes. Separate namespaces for types and values.
4. **Semantic analysis:** bidirectional local inference, lexical capture analysis,
   return-path checks, exact payload types, and finite effect bounds through
   direct and indirect calls. Reject before lowering. Nominal object/access
   checks, scenario eligibility summaries, and region restrictions are planned.
5. **Typed IR:** explicit basic blocks, typed local slots, temporaries, calls,
   branch targets, and terminators. Initially use mutable slots rather than SSA.
   Preserve source locations and effect signatures; future region qualifiers
   must survive until all relevant checks run.
   A later SSA optimization pass must not redefine language semantics.
6. **Lowering:** emit typed instructions for numeric widths, locals, branches,
   closures, direct/indirect calls, intrinsics, total result-tag dispatch, and
   observations. Array operations use typed built-in intrinsics with validated
   element/callback signatures. Object/scenario instructions are future extensions;
   those must encode resolved field/method IDs instead of VM source-name lookup.
7. **Verification:** check instruction indices, stack types at joins, initialized
   locals, branch targets, closure capture layouts, immutable capture slots,
   intrinsic types, call signatures, and effect bounds. Check unreachable code
   structurally as well. Before dataflow, preflight every type in the module,
   including forward-call targets. Bound depth to 128 edges and expanded size
   to 4096 nodes per type, counting shared subtrees repeatedly, so structural
   comparisons and diagnostic formatting remain bounded. Scenario nesting/escape
   and host capability validation must be added alongside those features.
8. **VM:** explicit value stack and call frames, checked arithmetic, tagged
   option/result/fixed-array values, and a traced array/closure heap. Collection
   callbacks suspend their caller in explicit continuation state. A LARP runtime
   trap is structured data, not a Rust panic. Future scenario frames must be
   discarded when a trap propagates through the VM loop.

The parser may be temporarily exercised without a backend, but the first
executable language slice must go all the way through typed IR and bytecode.
An AST walker may be a test oracle later; it is not the production execution path.

## Workspace boundaries

Current crates:

| Crate | Responsibility |
| --- | --- |
| `larp-source` | Immutable source text, validated spans, locations |
| `larp-diagnostics` | Structured diagnostic data and stable code explanations |
| `larp-types` | Scalar/composite types, typed intrinsics, checked operations, effect bounds |
| `larp-syntax` | Lexer, AST, recovering recursive-descent / Pratt parser |
| `larp-compiler` | Name, type, control-flow, and effect checking |
| `larp-ir` | Typed basic blocks and bytecode lowering |
| `larp-bytecode` | In-memory instructions, dataflow verifier, disassembly |
| `larp-vm` | Iterative frames/callbacks, tagged values, array/closure GC, checked execution, observations |
| `larp-cli` | `check`, `run`, `dump`, and `explain` |
| `larp-scenario-model` | Isolated experiment, not linked into the CLI |

Each crate contains working code. The dependency direction is from
CLI to compiler/runtime, never from semantic libraries to CLI. The VM and
bytecode format must not depend on AST types. Share only deliberately stable
artifact types between the emitter and loader.

Keep the initial workspace dependency-free. Evaluate libraries for CLI parsing,
diagnostic rendering, serialization, property tests, and fuzzing when needed.
A custom source location abstraction is small; a custom terminal layout engine
or cryptographic artifact format is not a good use of engineering time.

## Closures, effects, and result dispatch

Named functions become capture-free closures when used as values. Lambdas are
compiled to generated functions after lexical free-name analysis; capture slots
and generated code identities are explicit in typed IR and bytecode. Polymorphic
prelude references need an expected concrete function type and compile to typed
wrappers. Direct named calls remain direct instructions.

A function type contains invariant parameter/result types and a finite effect
bound. Assignment can widen that bound, while all later indirect calls check the
stored bound against the caller. Container payloads remain invariant. The
verifier checks the same relation; neither wrapping a callback nor storing a
function value removes its effects.

Recovery evaluates the result and handler expression before branching. A typed
`MatchResult` terminator lowers to two verified successors: one receives the
success payload, the other the error payload. Only the latter invokes the
handler. This avoids an unsafe unwrap operation or an artificial abort effect in
pure recovery. IR block parameters become typed stores at branch entry.

## Collections and places

Dynamic arrays carry shared heap identity. Fixed arrays are owned element vectors
with value-copy semantics; copying a reference element retains that element's
identity. Literal construction, indexing, indexed replacement, size, growth,
sorting, and higher-order operations lower to typed intrinsics. The same intrinsic
signature defines operand/result types and required effects for both the compiler
and bytecode verifier. Dynamic reads require `read`, mutation requires `write`,
and checked index/growth operations include `abort`. Fixed-value updates have no
heap mutation effect. Higher-order intrinsics add the callback's static effect
bound to receiver reads; there is no special route around effect checking.

An indexed assignment saves its place's indices and reference receivers and
validates each index before evaluating the right side. An invalid place aborts
before right-side effects; growth on the right cannot make it valid. Compound
assignment also reads the old element first.
Fixed-array ancestors are rebuilt through their saved places after evaluation,
preserving sibling updates made by the right side rather than writing back an
outdated whole-array snapshot. A captured fixed value cannot be reassigned;
references contained within it can still refer to mutable dynamic arrays.

`map`, `filter`, and `forEach` suspend the caller with an iteration record holding
receiver, callback, next index, current filter element, and unfinished output.
Each callback uses an ordinary VM frame. Returning resumes the iteration in the
VM loop, so nested pipelines do not recurse through the Rust stack. A per-array
iteration count rejects `push` before mutation even through aliases or captures.
Indexed replacement is nonstructural and remains allowed. Nested read-only
iterations of one receiver are permitted. Normal completion releases the guard;
a fatal trap terminates execution and releases the owning VM heap.

Filtering and mapping allocate new dynamic arrays; sorting copies the receiver
and uses stable integer/text ordering. Text length counts Unicode scalars.
Dynamic arrays have no equality; fixed-array equality recurses only through
element types that themselves support equality.

## Runtime representation and memory

The current VM uses tagged Rust `Value` variants: scalar values, tagged
option/result payloads, fixed arrays, nominal conversion errors, and array/closure
handles. Options, results, and fixed arrays own their payloads; tracing walks
through those wrappers to find contained handles. Text currently uses owned
immutable storage. Avoid NaN boxing until profiling proves a need.

Closures contain a generated function identity and copied captured values, never
Rust references into a VM stack. Ordinary parameters occupy the first frame
slots, followed by immutable capture slots and mutable locals/temporaries.
Nested and returned closures retain their captured values after the creating
frame exits. Capture bindings are snapshots; no source-level capture reassignment
is allowed. Captured dynamic arrays retain shared identity when copied.

Array/closure storage is a non-moving arena of optional typed entries with a
generation in each reusable handle. Stale generations cannot resolve to a newly
allocated entry; generation overflow is a checked resource failure. The collector
uses iterative marking and sweeping, including transitive handles in captured
options/results/fixed arrays. Mutable arrays can create cycles with closures;
unreachable cycles are reclaimed without recursive graph destruction.

Collection occurs before closure/array construction and sorting while operands
are still on the VM value stack, and before publishing completed callback
outputs while their continuation is still rooted. The complete stack,
initialized locals of all active frames, and each suspended iteration's receiver,
callback, current element, and output buffer are roots. Array elements and closure
captures are traced edges. Stress mode collects at every such safepoint; the
normal threshold adapts to the live heap. CLI `run --gc-stress` and the Rust
`Limits::gc_stress` option select stress mode. `execute_with_stats` exposes allocation,
collection, reclamation, and peak-live counts for tests. A final collection after
successful program exit reclaims the remaining array/closure heap. Trap paths
release the owning Rust heap as execution unwinds.

Object layouts, role dispatch tables, lazy cells, and scenario snapshots are
planned. Their fields and temporary allocation buffers must become roots or
traced edges before their allocation instructions can run. Objects will contain
class/layout identity and field values; role values will contain a receiver and
dispatch identity. Preserve stable language identity across collection and allow
unreachable cycles to be reclaimed. Reclamation timing is unobservable; bounded
pause times are not promised.

## Scenario implementation

A frame owns root identity, B, P, selected-field bits, claim values, and the
scenario label. Copy immutable/scalar field values at entry. Reference-bearing
roots are rejected. No recursive graph copying is hidden behind this operation.

The root name resolves to a `View_r` in the region environment. Value qualifiers
are propagated statically. Method invocation passes a tagged view receiver;
`self` access then uses the performed field vector. Never hand a scenario method
an ordinary root handle and hope it will avoid real mutations. Root-derived
references and other outer aliases cannot be passed, stored, or captured.

Finite effect flags alone cannot prove a call safe: `uses()` does not encode
reference allocation or a closure's reference captures. Compute transitive
scenario-eligibility summaries as well: reference allocation, receiver/argument
footprints, captures, returns, dispatch targets, and escaping stores. For v0.1,
reject unresolved cases. An indirect function type without a verified eligible
target set cannot be called from a scenario. Avoid pretending this is a complete
borrow checker or a general effect calculus.

Compile `flex`, `canon`, and `palantir` to dedicated operations. A final commit
validates the complete prepared state, then replaces the selected real values
without any intervening user code, allocation, or GC. Drop the frame on a VM trap.
Observation sinks are independent of field storage, so output is never rolled
back. Do not implement speculative writes by mutating the live object and trying
to reverse them afterward.

The separate model tests only immutable baseline, performed writes, last claims,
field selection, fallthrough commit, and discard. Rust borrowing in the model is
stronger than the future VM's identity model. It does not prove alias soundness
in LARP, effect inference, or region erasure safety.

## Bytecode artifacts

Begin with an internal instruction enum and in-memory verified modules. Before
`build` writes `.larpbc`, specify magic/version, bounded sections, integer byte
order, constant representations, function signatures, effect/region summaries,
source maps, and entry identity. No Rust enum discriminants or memory layouts
are an external file format. Bound lengths before allocation and reject unknown
versions. Do not deserialize arbitrary host objects.

The verifier is required before loading persisted modules. Compiler output is
not inherently trusted when a file can be edited. Runtime host functions check
capabilities as a second boundary, including active scenario restrictions.
Stack validation alone does not enforce scenario safety. Exact opcodes and
serialization are a later bytecode RFC; freezing them now would be premature.

## Diagnostics and tools

Semantic phases emit structured English diagnostics with stable codes. Rendering
into terminal text or future JSON happens at the boundary. Keep expected/found
types and related spans in data, not embedded in ANSI strings. Memes never replace
an actionable explanation. A formatter must operate on parsed syntax with comment
preservation; it is not a regex substitution pass.

Commands already available: `explain`, `check`, compiler-to-VM `run`, and
IR/bytecode `dump`. Next: verified artifact `build`/`run`; `fmt`; tooling integrations. No language
server, incremental query engine, package manager, or optimizer is needed before
basic programs pass conformance.
