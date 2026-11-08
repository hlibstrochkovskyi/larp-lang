# Bytecode artifact format, version 3

Status: codec implemented on `feature/patterns`; full language acceptance is
tracked by [RFC 0003](rfcs/0003-enums-patterns-and-propagation.md). This document
specifies wire representation, not a claim that every RFC example executes.

The encoder writes version 3. The decoder accepts versions 1, 2 and 3 and
independently verifies each reconstructed module. Other versions fail explicitly.
This remains an internal developer format, not a native executable or a package
format. Diagnostic and version-info JSON schemas do not change.

Version 3 retains every primitive encoding, section 1–7, type tag 0–14,
opcode 0–29 and intrinsic tag from [version 1](bytecode-format-v1.md) and
[version 2](bytecode-format-v2.md). Header offset 8 stores little-endian `u16(3)`.
The complete version-3 file contains sections 1 through 8 in that order. Section
8 is required even when the module declares no enums. Version-1/2 files retain
exactly seven sections and decode with an empty canonical enum table.

## Enum type tag 15

The type table remains topological. Every payload type reference names an earlier
type-table entry; enum declaration order and nominal IDs are independent of
that ordering. Recursive enum descriptor graphs cannot be encoded.

Type tag 15 has this payload, in order:

| Field | Encoding |
| --- | --- |
| Enum ID | `u32`, index in the module's canonical enum table |
| Display name | Existing length-prefixed UTF-8 string |
| Variant count | `u32`, 1 through 256 |
| Each variant | Name string, payload count `u32`, then payload type indices |

A variant has zero through 64 ordered payload types. Names must be nonempty;
variant names must be unique within the enum. Payload types must be well formed
and cannot be `unit` or internal lazy cells. Managed-reference payloads are
allowed; snapshot and equality eligibility inspect all variants rather than only
the currently active one. Type tag 15 is rejected under wire versions 1 and 2.

Enum descriptors participate in the existing expanded-type depth/node limits,
shared type-reference/work accounting, string limits and logical decoded
allocation budget. Names and variant names are charged as part of expanded work.
Variant and payload counts are checked before vector allocation.

## Canonical enum section 8

The section payload contains an enum count `u32` bounded by 4096, followed by that
many type-table indices. Every index must name an enum descriptor. A descriptor's
nominal ID must equal its position in this table. The decoder reconstructs
`Module.enums` from these descriptors.

The mandatory verifier validates all enum descriptors against this canonical
table, including descriptors nested in signatures, fields, containers and
unreachable instructions. The loader additionally checks unused type-table enum
entries against the same canonical table. An instruction cannot substitute a structurally
safer descriptor for the same nominal enum ID. Names, discriminants, field
arity/types and total dispatch targets remain independently checked.

## New opcodes

| Tag | Operation | Operands |
| --- | --- | --- |
| 30 | `MakeEnum` | Enum type index `u32`, variant index `u32` |
| 31 | `MatchSum` | Sum type index `u32`, target count `u32`, target PCs `u32[]` |

Both opcodes require wire version 3. All operand indices refer to decoded tables
or the current function's instruction PCs, as with existing operations.

`MakeEnum` consumes the selected variant's ordered payload fields and pushes one
enum value. Arguments are pushed left to right, so the final field is at the top
of the stack. The verifier requires exact payload types and a valid discriminant.
Construction itself has no semantic read/write/observation effect.

`MatchSum` consumes one finite-sum value and transfers control to exactly one
successor. Its target count must equal the number of alternatives. The successor
receives that alternative's ordered payload values; there is no unchecked
projection or nonexhaustive fallthrough. Alternative order is:

- Enums: declaration order.
- `baddie<T>`: `no_bitches`, `some(T)`.
- `result<T,E>`: `win(T)`, `fumble(E)`.
- `verdict`: `cap`, `fr`, both without payloads.

Target PCs must be valid instruction boundaries. Stack and definite-initialization
analysis checks each edge with only its selected payload types. New operations
also undergo regional validation; reference-bearing enum values cannot enter
scenarios through stack-only or unreachable operations. The existing opcode 15,
`MatchResult`, remains supported unchanged.

## Compatibility and execution

Old seven-section artifacts load without rewriting their bytes. Re-encoding them
uses version 3 and adds an empty enum table. Older readers reject a version-3
header as unsupported. Editing only the header cannot downgrade a new file:
section framing and new type/opcode gates are checked independently.

Codec tests cover explicit tags, truncations, prior-type references, bounded
layout counts, deterministic enum payload-dispatch roundtrips and mandatory
nominal verification. The [validation record](validation-patterns.md) identifies the candidate that
passed runtime acceptance, including a genuine frozen executable v3 fixture. Existing
version-1/2 compatibility fixtures retain their original source and artifact
hashes. See the [version policy](versioning.md) and RFC acceptance gates for the
full compiler/VM evidence required by this milestone.
