# Bytecode artifact format, version 2

Status: implemented. The encoder writes version 2; the loader accepts versions 1
and 2 and independently verifies both. Other versions fail explicitly. This
remains an internal developer format, not a native executable or package format.

Version 2 retains every section, primitive encoding, type tag, opcode, intrinsic
tag 0–19, source map, and resource budget from [version 1](bytecode-format-v1.md).
The header version at byte offset 8 is the little-endian `u16` value 2. The only
wire additions are two zero-operand intrinsic tags carried by opcode 14:

| Intrinsic tag | Operation | Parameters | Result | Effects |
| --- | --- | --- | --- | --- |
| 20 | Arguments | none | mfs<text> | io |
| 21 | ReadLine | none | result<baddie<text>,text> | io |

A version-1 file containing either new tag is malformed and rejected before VM
execution. Inheriting the wire version into every section reader prevents a
forged header from silently enabling version-2 operations under version 1.
Existing tag goldens and truncation tests also cover the new intrinsics.

A version-1 module using only its original tags loads unchanged. Re-encoding it
writes a version-2 header. The old version-1-only toolchain rejects new artifacts
as unsupported; rebuild/use the matching toolchain rather than editing a header.
The compatibility tests exercise old-tag files and verify that downgrading a
host-input file's header cannot bypass the version restriction.

Neither tag grants permission or carries host data. `execute_with_host` receives
arguments and stdin separately, and the legacy VM execution APIs deny them. Their
source effects, indirect callback bounds, constructor/lazy restrictions, and
scenario isolation are checked independently of wire validity. See the
[host input API](host-input.md) for runtime behavior and budgets.
