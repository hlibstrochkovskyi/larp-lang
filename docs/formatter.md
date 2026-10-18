# Source formatter

Status: implemented M6 tool. Its public API is
`larp_format::format(&SourceFile) -> Result<String, Vec<Diagnostic>>`. This document
records the current contract; see [executed validation](validation-formatter.md).

## Commands and failure

```sh
larp fmt program.larp --stdout
larp fmt program.larp --check
larp fmt program.larp
```

`--stdout` prints formatted source without modifying the file. `--check` writes
nothing and exits with 1 when formatting would change the file; it exits with 0
when the source is already formatted. Both flags may also precede the path.
The default mode replaces the source with its formatted text. Options are
mutually exclusive. Artifact input is rejected; there is no source rewriting
inside `.larpbc` files.

Formatting requires a syntactically valid complete program, including
`main_character`. It does not resolve names or check types/effects. Programs
with a wrong type or an unfinished role implementation may still be formatted;
lexical errors, missing punctuation, and parser resource failures are reported
with the existing structured diagnostics and do not modify files. No compiler,
VM, or user code executes during formatting.

The CLI computes the complete result before writing, writes a fresh sibling
temporary file, restores source permissions, flushes it, and publishes by rename.
Source symlinks are followed and preserved. An unchanged file is not rewritten.
The current source bytes are checked before publication; a detected intervening
edit aborts the replacement and removes the temporary file. This is not a file
lock or filesystem compare-and-swap: simultaneous editor changes must still be
coordinated. Rename follows host filesystem semantics; crash durability and
preservation of inode identity, hard-link relationships, ownership, or extended
attributes are not promised.

Read, parse, format, and write failures exit with 1. Invalid command usage exits
with 2. An internal preservation failure uses `LARP-C001` and leaves the original
source unchanged.

## Preservation and implementation

The formatter uses the real parser and lossless lexer. Lexer comment records
carry kind and source span; source slices supply original spellings. The compiler
continues using the ordinary token stream without recording comments.

AST spans provide syntax context, including generic angle brackets and unary
operators. A concrete delimiter tree retains tokens, parentheses, member order,
and declaration order. Formatting operates on that concrete stream rather than
printing the AST's grouped declaration vectors. It does not insert/remove
parentheses, canonicalize literals or escapes, reorder effects, change visibility,
replace `mogg`, or rewrite executable expressions.

Every successful result is parsed again. The original and formatted raw token
and comment sequences must agree before the result can reach the writer.
Comments remain opaque text in their original order, including Unicode and
embedded whitespace. String spellings, numeric separators, and explicit
parentheses are preserved. Formatting twice must yield the same bytes.

## Layout policy

The initial policy uses four-space indentation, blocks with line breaks, and a
100-Unicode-scalar soft target for delimiter lists. Commas and operators receive syntax
appropriate spacing; generic brackets differ from comparisons, and prefix signs
differ from binary arithmetic. Multiline list layout and comments can introduce
breaks without adding trailing commas (the grammar does not accept them).
Long opaque tokens and comments may exceed the soft target. There is no alignment
matrix, import sorting, configurable style, or formatter-disable pragma in this
slice. The tool policy can evolve separately from language semantics.

Brace blocks are multiline, including empty blocks. A list stays flat when its
measured width fits; a list with a multiline comment, nested block, or excessive
width breaks at commas, with its delimiters on separate lines and one additional
indentation level. Empty parentheses and brackets remain compact. The soft target
applies to delimiter groups, not arbitrary long expressions or method chains.
There is no terminal-cell width calculation for combining/wide characters.

Existing blank gaps in statement/block sequences collapse to one blank line.
Bare top-level block declarations receive a separating blank line. Inline comments
remain with their preceding token when the source gap has no LF, including after
an opening delimiter, comma, or semicolon. Standalone comments remain standalone.
Line comments always terminate a line; a multiline block comment is copied
verbatim and followed by a line break. The layout outside comments uses LF;
embedded CRLF and the CR at the end of an original CRLF line comment remain
inside that comment's raw span. The result ends with a final LF.

For example:

```larp
main_character {
    mf n = 4;
    if (n mogg 0) {
        yap(n);
    } else {
        yap(0);
    }
}
```

## Resource policy

Input and output are each limited to 4 MiB. The CLI reads at most this limit
plus one sentinel byte before UTF-8 decoding and source-line indexing. Equal byte limits ensure that every
successful output can be passed to the formatter again. Layout expansion beyond
the output budget fails without returning partial output. At most 250,000 lexical
tokens plus comments are admitted before AST construction; the EOF token counts
toward this budget. The concrete delimiter tree has a 512-level implementation
limit, in addition to the parser's existing nesting/expression limits. Resource
errors use `LARP-P003`; preservation failures use `LARP-C001`.

These are logical size/work limits, not bounds on process RSS or wall-clock time.
Lexing must allocate its token records before the token count can be checked;
parser/tree/rendering and preservation checks have additional temporary storage.
Coverage-guided fuzzing and whole-process resource measurements remain preview
work. See [validation](validation-formatter.md) for the checks already executed.
