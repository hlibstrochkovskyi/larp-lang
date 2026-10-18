# Bytecode artifact format, version 1

Status: implemented internal developer format. The wire version is independent
of the language specification and Cargo package version. Changes to these tags,
field orders, or meanings require a new wire version; unsupported versions fail
explicitly. Long-term distribution compatibility is not promised by this preview.

`larp-artifact` serializes an already verified module and embeds its source map.
Loading always reconstructs and independently verifies the complete module before
the VM receives a `VerifiedModule`. Files contain neither a live heap nor host
objects, capabilities, executable native code, or typed IR.

## Primitive encodings and framing

All integers are little-endian. An index, count, byte length, and fixed-array
length is a checked `u32`. A Boolean is exactly one byte, 0 or 1. Strings are a
`u32` byte length followed by UTF-8 bytes without a terminator. Vectors begin
with a `u32` element count. An optional index is a Boolean followed by an index
only when present. Effects are one byte: read=1, write=2, observe=4, io=8,
abort=16. Other bits are rejected.

The 16-byte header is:

| Offset | Width | Value |
| --- | --- | --- |
| 0 | 8 | ASCII `LARPBC` followed by CR LF |
| 8 | 2 | Wire version 1 |
| 10 | 2 | Flags 0 |
| 12 | 4 | Exact payload byte length |

Each required section is a byte ID, `u32` byte length, and that many bytes.
Sections must occur exactly once, in this order. Unknown, reordered, missing,
truncated, or trailing sections/bytes are errors. Individual sections must also
be consumed completely.

| ID | Contents |
| --- | --- |
| 1 | Vector of sources: source ID, name string, source text string |
| 2 | Vector of types described below |
| 3 | Vector of roles: name, vector of methods |
| 4 | Vector of classes: name, constructor index, fields, methods, implementations |
| 5 | Vector of extensions: receiver type ID, name, function index |
| 6 | Vector of functions described below |
| 7 | Entry function index |

Source IDs are unique `u32` values. A span contains source ID, start byte, and
end byte, in that order. Every function/instruction span must reference an
embedded source and a valid half-open UTF-8 byte range. Embedded names are
presentation data; the loader never opens those paths. Runtime diagnostics still
work after the original source has moved or been removed. Source text is embedded
in full; an artifact therefore includes the program's source, comments, and names.

## Types and nominal tables

Type children reference earlier entries in the type table. Cycles and forward
references are rejected. The deterministic encoder interns types in first
encounter postorder; it never serializes hash-table iteration order. Duplicate
structurally equal entries are permitted on input.

| Tag | Type | Operands after tag |
| --- | --- | --- |
| 0 | mf | none |
| 1 | unc | none |
| 2 | geeked | none |
| 3 | verdict | none |
| 4 | text | none |
| 5 | unit | none |
| 6 | function | vector of parameter type IDs, result type ID, effects |
| 7 | baddie | payload type ID |
| 8 | result | success type ID, error type ID |
| 9 | dynamic array | element type ID |
| 10 | fixed array | element type ID, length |
| 11 | ConversionError | none |
| 12 | class | nominal class index, name string |
| 13 | role | nominal role index, name string |
| 14 | internal lazy cell | result type ID, effects |

Well-formedness, exact nominal IDs/names, and restrictions on internal lazy types
are checked independently of encoding. Declared role methods contain name,
parameter type-ID vector, result type ID, and effects. Class fields contain
name, type ID, and public Boolean. Class methods contain name, function index,
and public Boolean. Implementations contain role index and a vector of method
function indices in the role's declaration order. Extensions retain their exact
static receiver type and do not grant class ownership or private access.

## Functions and instructions

Function fields, in order:

1. Scenario-present Boolean; when present: entry index, class index, optional
   ordinary origin function index.
2. Optional owner class index and optional constructor class index.
3. Name string.
4. Parameter type-ID vector, capture type-ID vector, result type ID, effects.
5. Local type-ID vector, function span, instruction vector.

Each instruction is an opcode with its operands, followed by a span. Instruction
indices are local to their enclosing function. The loader preserves ownership,
region origins, private access, closure layouts, and effect bounds. No metadata
is treated as a trusted summary that can bypass instruction verification.

| Opcode | Operation | Operands after opcode |
| --- | --- | --- |
| 0 | Constant | scalar |
| 1 | Load | local index |
| 2 | Store | local index |
| 3 | Unary | unary tag, operand type ID |
| 4 | Binary | binary tag, operand type ID |
| 5 | Jump | instruction index |
| 6 | JumpIfFalse | instruction index |
| 7 | Call | function index |
| 8 | MakeClosure | function index |
| 9 | Construct | class index |
| 10 | NewObject | class index |
| 11 | GetField | class index, field index |
| 12 | SetField | class index, field index |
| 13 | CallIndirect | function type ID |
| 14 | Intrinsic | intrinsic tag and operands |
| 15 | MatchResult | result type ID, success instruction index, error instruction index |
| 16 | Return | none |
| 17 | Print | none |
| 18 | Assert | none |
| 19 | Crash | none |
| 20 | IntoRole | class index, role index |
| 21 | CallRole | role index, method slot |
| 22 | Scenario | entry function index, label string |
| 23 | Canon | class index, field index |
| 24 | Flex | class index, field index |
| 25 | Palantir | class index, vector of visible field indices |
| 26 | MakeLazy | function index |
| 27 | Force | internal lazy type ID |
| 28 | SilenceEnter | none |
| 29 | SilenceExit | none |

Scalar tags are 0 mf (`i32`), 1 unc (`i64`), 2 geeked (raw `u64` IEEE bits),
3 verdict (Boolean), 4 text (string), and 5 unit (no payload). NaN payloads,
infinities, and negative zero retain their bits. Source literal restrictions
remain frontend rules.

Unary tags are 0 negate, 1 positive, 2 not. Binary tags are 0 add, 1 subtract,
2 multiply, 3 divide, 4 remainder, 5 equal, 6 not equal, 7 less, 8 less/equal,
9 greater, 10 greater/equal, 11 and, 12 or.

Intrinsic tags 0–6 are None, Some, Ok, Err, IsSome, IsOk, Copium, each followed
by its type ID. Tag 7 ParseScore has no operands. Tag 8 Convert has source type
ID, destination type ID, and fallible Boolean. Tag 9 ConversionMessage has no
operands. Tag 10 ArrayNew has array type ID and element count. Tags 11–16 are
Size, Index, CheckIndex, SetIndex, Push, Sorted, each followed by its type ID.
Tags 17–19 are Map, Filter, ForEach, each followed by array type ID and callback
type ID. Numeric tags are explicit contracts, never Rust enum discriminants.
The exhaustive codec and tag tests are in `larp-artifact/src/codec.rs`.

## Resource policy and failure

The current loader checks lengths against both policy and remaining bytes before
reserving vectors/strings. Allocation uses checked arithmetic and fallible
reservation where supported. Budgets are shared across sections:

| Resource | Limit |
| --- | --- |
| Whole file, including header | 16 MiB |
| Sources | 64 |
| Total source text | 4 MiB |
| Total source lines, including each initial line | 131,072 |
| Individual non-source string | 1 MiB |
| Total decoded string bytes | 8 MiB |
| Functions, classes, roles, extensions | 4,096 each |
| Nominal/inspection member and implementation table entries | 65,536 total |
| Type entries | 65,536 |
| Decoded type references | 262,144 |
| Expanded type traversal work | 2,000,000 expanded nodes plus repeated nominal-name UTF-8 bytes |
| Each type | 128 nested edges; 4,096 expanded nodes |
| Instructions | 262,144 total |
| Local slots | 65,536 total |
| Logical decoded allocation charges | 64 MiB |

The 64 MiB charge covers decoded vector capacities, string storage, source line
indices, and conservative type-node allocation allowances. It is not a bound on
process RSS, allocator overhead, verifier memory, or VM heap usage. File input
and loaded data can coexist. The bytecode verifier also limits stored dataflow
cells to 4,000,000 per function and processed analysis cells to 16,000,000 across
the module, including revisits. Signature/type preflight separately permits 16,000,000
expanded nodes plus nominal-name bytes, including parameters derived by
intrinsics rather than explicitly encoded in the file. Complete function headers, including captures,
share the type complexity bound. These are resource policies, not wall-clock
execution guarantees. VM fuel and frame limits apply separately when running.

Encoding first emits bounded bytes, then loads and verifies them with the same
policy. This deliberately duplicates verification to guarantee that a successful
build produces a loadable artifact; it has an additional build-time cost.
There is no compression, checksum, authentication, or source-to-binary attestation.
Structural validity does not establish who authored a file.

Malformed/unsupported framing uses `LARP-B002`; resource failures use
`LARP-B003`; semantically invalid bytecode uses `LARP-B001`. Loader errors retain
an optional byte offset; verification errors retain function/instruction indices.
No VM output occurs before successful loading and verification.

## CLI and output publication

`build FILE.larp [-o OUTPUT]` defaults to the source path with `.larpbc` extension.
It compiles without execution, encodes, writes a fresh sibling temporary file,
flushes it, and renames the completed file into place. Failed compilation or
encoding preserves previous output. Temporary files are removed on write/rename
failure. Direct output to the input source, including a resolved symlink alias,
is rejected. Atomic replacement follows host rename semantics; directory fsync
and recovery after abrupt power loss are not promised.

`run`, `check`, and `dump` detect artifacts by magic or `.larpbc` extension.
Artifact `check` verifies without execution, `dump` disassembles loaded bytecode,
and `run` honors the usual fuel/GC options. `dump --ir` and `build` require source.
Missing files, compile errors, load errors, and runtime errors exit with 1;
invalid command usage exits with 2. `fmt` remains unavailable.
