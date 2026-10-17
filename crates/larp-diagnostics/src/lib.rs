//! Structured diagnostics and explanations shared by future compiler phases.

use larp_source::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticCode {
    pub id: &'static str,
    pub title: &'static str,
    pub explanation: &'static str,
}

pub const CODES: &[DiagnosticCode] = &[
    DiagnosticCode {
        id: "LARP-R008",
        title: "Checked conversion failed",
        explanation: "fraud<T>(value) could not represent the integer in its target type. Use ascend<T>(value) and handle the result with cope for recoverable range errors.",
    },
    DiagnosticCode {
        id: "LARP-B001",
        title: "Invalid bytecode",
        explanation: "Bytecode verification failed. The VM only accepts modules whose control flow, operand types, local initialization, call signatures, and effect bounds have been validated. Compiler-generated verification failures are compiler bugs.",
    },
    DiagnosticCode {
        id: "LARP-T009",
        title: "Invalid control flow",
        explanation: "The statement is not valid in this control-flow context. touch_grass requires an enclosing loop; return must agree with the function's result type.",
    },
    DiagnosticCode {
        id: "LARP-L001",
        title: "Invalid source token",
        explanation: "The source contains an invalid character, malformed numeric literal, string escape, or unterminated string/comment. Source text must be UTF-8.",
    },
    DiagnosticCode {
        id: "LARP-P002",
        title: "Feature is not implemented yet",
        explanation: "This construct belongs to a later compiler milestone. It is rejected explicitly rather than given incomplete or different semantics. See the implementation status document.",
    },
    DiagnosticCode {
        id: "LARP-P003",
        title: "Compilation resource limit exceeded",
        explanation: "The source exceeds a parser nesting/expression limit or a compiler/verifier resource budget. Split large expressions and functions. This is an implementation resource limit, not a type error or malformed-bytecode compiler bug.",
    },
    DiagnosticCode {
        id: "LARP-P001",
        title: "Unexpected syntax",
        explanation: "The token sequence does not match the supported grammar. Check the highlighted token and the expected syntax. Unsupported language features must be reported explicitly.",
    },
    DiagnosticCode {
        id: "LARP-N001",
        title: "Unknown name",
        explanation: "The name is not declared in this scope. Local bindings become visible after their initializer. Type and value names use separate namespaces.",
    },
    DiagnosticCode {
        id: "LARP-N002",
        title: "Duplicate declaration",
        explanation: "The same name was declared twice in one scope. Inner scopes may shadow outer bindings, but duplicate parameters, locals, or top-level function names are rejected.",
    },
    DiagnosticCode {
        id: "LARP-T004",
        title: "Invalid constant expression",
        explanation: "A numeric literal or evaluated literal-only operation is outside its type's range, divides by zero, or produces an invalid literal. Short-circuited operands are still type-checked but are not evaluated.",
    },
    DiagnosticCode {
        id: "LARP-T005",
        title: "Unknown type",
        explanation: "The type is not declared or is unavailable in the implemented language subset. API parameters and results require explicit types.",
    },
    DiagnosticCode {
        id: "LARP-T006",
        title: "Invalid function arguments",
        explanation: "The call supplies the wrong number of arguments. Function parameters have fixed arity; there are no implicit optional or variadic arguments in the current core.",
    },
    DiagnosticCode {
        id: "LARP-T007",
        title: "Missing return value",
        explanation: "A non-unit function has a reachable path that falls through without returning a value. Return a value on every path or declare a unit result.",
    },
    DiagnosticCode {
        id: "LARP-T008",
        title: "Read-only binding",
        explanation: "This binding cannot be assigned. Range-loop indices and captured bindings are read-only. Captures copy values when a closure is created. Use a separate mutable local for independent state.",
    },
    DiagnosticCode {
        id: "LARP-E003",
        title: "Unknown effect",
        explanation: "The uses(...) list may contain only read, write, observe, io, and abort. Bounds compose through calls and are checked against the function body.",
    },
    DiagnosticCode {
        id: "LARP-T001",
        title: "Incompatible types",
        explanation: "The expected and actual types disagree. LARP does not implicitly convert numbers, wrap options, or mix generic container element types.",
    },
    DiagnosticCode {
        id: "LARP-T002",
        title: "Condition requires verdict",
        explanation: "An if or while condition must have type verdict. Numbers, text, options, and objects have no implicit truthiness. Compare a number explicitly, for example aura mogg 0.",
    },
    DiagnosticCode {
        id: "LARP-T003",
        title: "Insufficient type information",
        explanation: "The initializer does not determine a complete type. Add an explicit type for an empty array, no_bitches, an unconstrained result variant, or a lambda parameter.",
    },
    DiagnosticCode {
        id: "LARP-T010",
        title: "Private member access",
        explanation: "A lowkey member is accessible only in the declaring class. Extensions, flex, and canon do not bypass this rule. Expose an appropriate public operation or change the member's declared visibility.",
    },
    DiagnosticCode {
        id: "LARP-T041",
        title: "MOGGED: implicit integer narrowing",
        explanation: "An unc (i64) value cannot be assigned implicitly to mf (i32). Use matching integer types, or ascend<mf>(value) returning result<mf, ConversionError> and handle failure with .cope(handler). fraud<mf>(value) performs the same range check but aborts on failure.",
    },
    DiagnosticCode {
        id: "LARP-E001",
        title: "Effect exceeds declared bound",
        explanation: "The body or a transitive call performs an effect outside its uses(...) bound. Indirect calls, role dispatch, and collection callbacks preserve effect bounds; they cannot hide I/O or mutation.",
    },
    DiagnosticCode {
        id: "LARP-E002",
        title: "Operation is not scenario-safe",
        explanation: "A larp block cannot perform external I/O, allocate reference objects, or call an operation without a verified scenario-safe summary. Observations are allowed but are never rolled back.",
    },
    DiagnosticCode {
        id: "LARP-S001",
        title: "Invalid scenario root",
        explanation: "A scenario requires one local binding of a concrete class whose fields are all SnapshotValues. Mutable arrays, nested object references, roles, and closures are not eligible root fields.",
    },
    DiagnosticCode {
        id: "LARP-S002",
        title: "Outer reference in scenario",
        explanation: "Only the named root's scenario view is available as an outer reference. Other object aliases, arrays, functions, and lazy bindings cannot be captured from outside. Compute a value snapshot before entering the block.",
    },
    DiagnosticCode {
        id: "LARP-S003",
        title: "Nested scenario",
        explanation: "Nested larp blocks are not supported in v0.1. Run scenarios sequentially; a later block observes the real state left by the previous block.",
    },
    DiagnosticCode {
        id: "LARP-S004",
        title: "Scenario value escape",
        explanation: "Regional values cannot be stored in outer bindings or external objects. Use canon root.field to select a final field value, or yap/palantir for explicit observation. canon does not select arbitrary local results.",
    },
    DiagnosticCode {
        id: "LARP-S005",
        title: "Control flow crosses scenario boundary",
        explanation: "return and loop exits cannot leave a larp block. Only normal fallthrough commits selected fields. A loop entirely inside the block may still use touch_grass.",
    },
    DiagnosticCode {
        id: "LARP-S006",
        title: "Invalid scenario operation",
        explanation: "flex and canon require a direct accessible field of the active root. palantir requires that root. These operations are not available outside a scenario and cannot select nested paths.",
    },
    DiagnosticCode {
        id: "LARP-R001",
        title: "Integer overflow",
        explanation: "The mathematical result does not fit the declared integer width. Integer operations are checked in every build mode. Active scenarios are discarded before fatal termination.",
    },
    DiagnosticCode {
        id: "LARP-R002",
        title: "Integer division by zero",
        explanation: "An integer division or remainder used zero as divisor. Return an explicit result for expected invalid input instead of relying on a fatal runtime check.",
    },
    DiagnosticCode {
        id: "LARP-R003",
        title: "Fatal invariant failure",
        explanation: "crashout or no_cap terminated the program. There is no user exception handler. Active scenarios discard performed state; previous observational output remains visible.",
    },
    DiagnosticCode {
        id: "LARP-R004",
        title: "Collection index or length out of range",
        explanation: "An array index was negative or outside its bounds, or a collection length exceeded 2147483647. Indices have type mf. Indexed assignments validate the place before evaluating the right side. No unchecked access or implicit conversion is performed.",
    },
    DiagnosticCode {
        id: "LARP-R005",
        title: "Array structurally mutated during iteration",
        explanation: "A collection callback attempted to grow or structurally change its receiver while iteration was active. The runtime guard applies through all aliases of that array.",
    },
    DiagnosticCode {
        id: "LARP-R006",
        title: "Execution resource limit exceeded",
        explanation: "Execution exceeded its configured instruction budget or call-frame limit. Increase the explicit VM limit for a trusted workload, or reduce recursion and computation. No Rust stack recursion is used for LARP calls.",
    },
    DiagnosticCode {
        id: "LARP-R007",
        title: "Observation sink failed",
        explanation: "The host could not write observational output. Execution stops with a structured runtime error instead of a host panic.",
    },
];

pub fn lookup(id: &str) -> Option<&'static DiagnosticCode> {
    CODES.iter().find(|code| code.id == id)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: &'static DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub primary: Label,
    pub related: Vec<Label>,
    pub expected_type: Option<String>,
    pub found_type: Option<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(id: &str, span: Span, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            code: lookup(id).expect("internal diagnostic IDs must be registered"),
            severity: Severity::Error,
            primary: Label {
                span,
                message: message.clone(),
            },
            message,
            related: Vec::new(),
            expected_type: None,
            found_type: None,
            help: None,
        }
    }
}

/// Plain text rendering; source columns count scalars and tabs expand to four spaces.
pub fn render(diagnostic: &Diagnostic, source: &larp_source::SourceFile) -> String {
    use std::fmt::Write;
    let mut output = format!("error[{}]: {}\n", diagnostic.code.id, diagnostic.message);
    fn label(output: &mut String, item: &Label, source: &larp_source::SourceFile) {
        if source.slice(item.span).is_none() {
            return;
        }
        let range = item.span.range();
        let Some(location) = source.location(range.start) else {
            return;
        };
        let line_start = source.text()[..range.start]
            .rfind('\n')
            .map_or(0, |i| i + 1);
        let line_end = source.text()[range.start..]
            .find('\n')
            .map_or(source.text().len(), |i| range.start + i);
        let line = source.text()[line_start..line_end].trim_end_matches('\r');
        let indentation = source.text()[line_start..range.start]
            .chars()
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum::<usize>();
        let width = source.text()[range.start..range.end.min(line_end)]
            .chars()
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum::<usize>()
            .max(1);
        let _ = writeln!(
            output,
            " --> {}:{}:{}",
            source.name(),
            location.line,
            location.column
        );
        let _ = writeln!(
            output,
            "{:>4} | {}",
            location.line,
            line.replace('\t', "    ")
        );
        let _ = writeln!(
            output,
            "     | {}{} {}",
            " ".repeat(indentation),
            "^".repeat(width),
            item.message
        );
    }
    label(&mut output, &diagnostic.primary, source);
    for related in &diagnostic.related {
        label(&mut output, related, source);
    }
    if let (Some(expected), Some(found)) = (&diagnostic.expected_type, &diagnostic.found_type) {
        let _ = writeln!(output, "expected: {expected}; found: {found}");
    }
    if let Some(help) = &diagnostic.help {
        let _ = writeln!(output, "help: {help}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_has_unique_well_formed_codes_and_explanations() {
        let mut ids = HashSet::new();
        for code in CODES {
            let bytes = code.id.as_bytes();
            assert_eq!(bytes.len(), 9);
            assert!(code.id.starts_with("LARP-"));
            assert!(b"LPNTESRCB".contains(&bytes[5]));
            assert!(bytes[6..].iter().all(u8::is_ascii_digit));
            assert!(ids.insert(code.id));
            assert!(!code.title.is_empty());
            assert!(!code.explanation.is_empty());
            assert_eq!(lookup(code.id), Some(code));
        }
        assert!(lookup("LARP-T999").is_none());
    }
}
