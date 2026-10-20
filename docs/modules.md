# File modules

Status: experimental **`0.2.0-dev`** functionality. Imports are absent from the
published `v0.1.0-preview.1` binary. Build this branch before trying these commands.
The [RFC](rfcs/0001-file-modules.md) defines the full contract; the
[project fixture](../tests/projects/README.md) exercises it through the real VM.

## Split a program

Save this entry as `main.larp`:

```larp
import "lib/math.larp" as math;

main_character {
    yap(math::twice(21));
}
```

Save its library as `lib/math.larp`:

```larp
highkey sidequest twice(mf value) -> mf {
    return value * 2;
}
```

Run `target/debug/larp run main.larp` to print `42`. Imports precede declarations.
The entry requires one `main_character`; libraries contain declarations and no
entry block. Every reachable library is checked, even when its exports are unused.
Imports do not execute code or introduce runtime initialization order.

## Names and visibility

Declarations are private to their file unless marked `highkey`; `lowkey` is
explicit privacy. Qualify an exported function or type with its import alias:
`math::twice`, `people::Founder`, or `roles::Mogger`. Dot access still selects
members on values. Exporting a class exposes its constructor and public members;
its private members retain ordinary lexical class access checks.

Aliases occupy a separate qualifier namespace: a local `mf math = 7;` does not
hide `math::twice`. Each file owns its aliases. A caller cannot traverse another
file's imports as `a::b::name`, and there are no re-exports or wildcard imports.

A file reached through several paths has one canonical identity. Two distinct
files declaring `Founder` create distinct nominal types, even if their contents
are identical. Functions, closures, lazy bodies, and extensions resolve names in
the file where they are defined. An exported signature must expose only public
nominal types, recursively through arrays, options/results, and function types.
Private implementation fields and helpers may use private types.

## Explicit imported extensions

Extensions declared in a file participate in that file's dot lookup. Importing
another file does not add its extensions to dot lookup. Call an exported extension
explicitly with `extensions::doubled(value)`; the receiver is the first argument.
Real members keep priority in ordinary dot lookup, including access errors.

The receiver uses its exact static type, with the same inference as a dot call.
An unconstrained integer receiver literal defaults to `mf`, even if only an `unc`
extension exists. Use an `unc` binding or explicit conversion for that receiver.
There is no implicit numeric or class-to-role conversion during receiver selection.
Remaining arguments use the selected signature's ordinary contextual typing.
The receiver and arguments evaluate once, from left to right.

A saved `extensions::doubled` function includes its receiver parameter. One
exported candidate can be inferred; several receiver candidates need an explicit
function type selecting one compatible signature. Its effect bound may widen.
An exported ordinary function and an exported extension cannot share a basename
in one file (`LARP-M007`); distinct extension receiver types may share one.
Extensions gain no access to class-private members. Cross-file calls and function
values preserve all effect and scenario restrictions.

## Paths, artifacts, and formatting

The CLI uses the canonical entry file's parent as the project root. Imports are
relative to their importing file and remain within that root after symlink
resolution. Use `/` paths ending in `.larp`; absolute paths, backslashes, NULs,
missing/nonregular files, and escaping symlinks are rejected. `..` is accepted
only within the root. There is no package registry or network lookup.

```sh
target/debug/larp check tests/projects/pitch/main.larp
target/debug/larp run tests/projects/pitch/main.larp --gc-stress
target/debug/larp build tests/projects/pitch/main.larp -o /tmp/pitch.larpbc
target/debug/larp run /tmp/pitch.larpbc --gc-stress
target/debug/larp fmt tests/projects/pitch/lib/founder.larp --stdout
```

One `.larpbc` contains the whole checked graph and original source maps. Running
it requires no original files or resolver. The existing mandatory bytecode
verifier still checks the program; the artifact wire version remains 2.
`fmt` processes one source unit at a time and can format a library without its
dependencies. Graph limits are 64 files, 4096 edges, 4 MiB total UTF-8 bytes, and
131072 source lines, in addition to existing compiler/artifact budgets.

Module diagnostics are `M001` resolution, `M002` cycles, `M003` duplicate aliases,
`M004` entry placement, `M005` aggregate graph limits, `M006` private types exposed
by public interfaces, and `M007` ambiguous exported names. Use
`larp explain LARP-M006` or JSON diagnostics for details and original source spans.
An unavailable private name uses the ordinary name/type diagnostic. The CLI's
per-file read cap is an I/O resolution error (`M001`); aggregate budgets use `M005`.

Packages, cyclic imports, module globals/initializers, separate compilation,
re-exports, and runtime module loading remain future work.
