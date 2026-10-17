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
        explanation: "An unc (i64) value cannot be assigned implicitly to mf (i32). Use ascend<mf>(value), which returns result<mf, ConversionError>, and handle failure with cope. Use fraud<mf>(value) only when a fatal range check is intended.",
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
        title: "Invalid array index",
        explanation: "An array index was negative or outside its bounds. Indices have type mf. No unchecked access or implicit conversion is performed.",
    },
    DiagnosticCode {
        id: "LARP-R005",
        title: "Array structurally mutated during iteration",
        explanation: "A collection callback attempted to grow or structurally change its receiver while iteration was active. The runtime guard applies through all aliases of that array.",
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
            assert!(b"LPNTESRC".contains(&bytes[5]));
            assert!(bytes[6..].iter().all(u8::is_ascii_digit));
            assert!(ids.insert(code.id));
            assert!(!code.title.is_empty());
            assert!(!code.explanation.is_empty());
            assert_eq!(lookup(code.id), Some(code));
        }
        assert!(lookup("LARP-T999").is_none());
    }
}
