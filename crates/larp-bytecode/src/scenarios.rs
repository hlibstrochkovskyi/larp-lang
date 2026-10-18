//! Regional frame restrictions for isolated snapshots and selective commit.

use crate::{Module, Op, ScenarioContext, VerifyError, VerifyErrorKind, classes};
use larp_types::{Effects, Type};

fn invalid(function: usize, instruction: Option<usize>, message: &str) -> VerifyError {
    VerifyError {
        kind: VerifyErrorKind::Invalid,
        function,
        instruction,
        message: message.into(),
    }
}

fn value_result(ty: &Type) -> bool {
    *ty == Type::Unit || ty.is_snapshot()
}

fn value_callable(ty: &Type) -> bool {
    matches!(ty, Type::Function(signature)
        if signature.parameters.iter().all(Type::is_snapshot)
        && value_result(&signature.result) && !signature.effects.contains(Effects::IO))
}

fn root_type(module: &Module, context: ScenarioContext) -> Type {
    Type::class(context.class, module.classes[context.class].name.as_str())
}

pub(super) fn validate_contexts(module: &Module) -> Result<(), VerifyError> {
    for (index, function) in module.functions.iter().enumerate() {
        let Some(context) = function.scenario else {
            continue;
        };
        let bad = |message| invalid(index, None, message);
        let Some(class) = module.classes.get(context.class) else {
            return Err(bad("Scenario root class index out of bounds"));
        };
        let Some(entry) = module.functions.get(context.entry) else {
            return Err(bad("Scenario entry index out of bounds"));
        };
        if !matches!(entry.scenario, Some(entry_context) if entry_context.entry == context.entry
            && entry_context.class == context.class && entry_context.origin.is_none())
        {
            return Err(bad("Scenario context does not match its entry"));
        }
        if let Some(origin) = context.origin {
            let source = module
                .functions
                .get(origin)
                .ok_or_else(|| bad("Scenario origin function index out of bounds"))?;
            if source.scenario.is_some()
                || source.constructor.is_some()
                || source.owner != function.owner
                || source.parameters != function.parameters
                || source.captures != function.captures
                || source.result != function.result
                || source.effects != function.effects
            {
                return Err(bad(
                    "Scenario specialization must preserve its ordinary origin signature and ownership",
                ));
            }
        }
        let root = root_type(module, context);
        if entry.parameters.first() != Some(&root)
            || entry.parameters[1..].iter().any(|ty| !ty.is_snapshot())
            || !entry.captures.is_empty()
            || entry.result != Type::Unit
        {
            return Err(bad(
                "Scenario entry requires one root, value snapshots, no captures, and unit result",
            ));
        }
        if class.fields.iter().any(|field| !field.ty.is_snapshot()) {
            return Err(bad("Scenario root fields must be snapshot values"));
        }
        if function.constructor.is_some() || function.effects.contains(Effects::IO) {
            return Err(bad("Scenario functions cannot be constructors or use IO"));
        }
        if module
            .classes
            .iter()
            .any(|class| class.methods.iter().any(|method| method.function == index))
            || module
                .extensions
                .iter()
                .any(|extension| extension.function == index)
        {
            return Err(bad(
                "Scenario functions cannot be registered ordinary members",
            ));
        }
        if function.parameters.iter().filter(|ty| **ty == root).count() > 1
            || function
                .parameters
                .iter()
                .any(|ty| *ty != root && !ty.is_snapshot() && !value_callable(ty))
        {
            return Err(bad(
                "Scenario parameters permit at most one root and value-only arguments",
            ));
        }
        if function.captures.iter().any(|ty| !ty.is_snapshot()) {
            return Err(bad("Scenario closures may capture only snapshot values"));
        }
        if !value_result(&function.result) {
            return Err(bad(
                "Scenario functions cannot return references or function values",
            ));
        }
        if function.locals.iter().any(|ty| {
            *ty != root && !value_result(ty) && !value_callable(ty) && !matches!(ty, Type::Lazy(..))
        }) {
            return Err(bad(
                "Scenario locals cannot contain external reference types",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_instruction(
    module: &Module,
    index: usize,
    pc: usize,
) -> Result<(), VerifyError> {
    let function = &module.functions[index];
    let op = &function.code[pc].op;
    let bad = |message| invalid(index, Some(pc), message);
    match op {
        Op::Scenario { entry, label } => {
            if function.scenario.is_some() {
                return Err(bad("Nested scenarios are forbidden"));
            }
            let target = module
                .functions
                .get(*entry)
                .ok_or_else(|| bad("Scenario entry index out of bounds"))?;
            if !matches!(target.scenario, Some(context) if context.entry == *entry) {
                return Err(bad("Scenario instruction requires a regional entry target"));
            }
            if label.is_empty() {
                return Err(bad("Scenario label cannot be empty"));
            }
            if function.constructor.is_some() {
                return Err(bad("Constructors cannot start scenarios"));
            }
            // Scenario entries inherit the lexical ownership of their source block.
            if target.owner.is_some() && target.owner != function.owner {
                return Err(bad("Scenario entry must preserve lexical ownership"));
            }
        }
        Op::Call(target_index) | Op::MakeClosure(target_index) | Op::MakeLazy(target_index) => {
            let target = module.functions.get(*target_index).ok_or_else(|| {
                bad(if matches!(op, Op::Call(_)) {
                    "Call target out of bounds"
                } else {
                    "Closure target out of bounds"
                })
            })?;
            let same_region = match (target.scenario, function.scenario) {
                (None, None) => true,
                (Some(target), Some(caller)) => {
                    target.entry == caller.entry && target.class == caller.class
                }
                _ => false,
            };
            if !same_region {
                return Err(bad("Calls and closures cannot cross scenario regions"));
            }
            if let Some(context) = target.scenario
                && context.entry == *target_index
            {
                return Err(bad("Scenario entries require the Scenario instruction"));
            }
            if function.scenario.is_some() && matches!(op, Op::MakeClosure(_)) {
                let signature = Type::function(
                    target.parameters.clone(),
                    target.result.clone(),
                    target.effects,
                );
                if !value_callable(&signature) {
                    return Err(bad(
                        "Scenario function values require value-only signatures",
                    ));
                }
            }
        }
        Op::Canon { class, field } | Op::Flex { class, field } => {
            let context = function
                .scenario
                .ok_or_else(|| bad("Scenario operation requires a regional frame"))?;
            if *class != context.class {
                return Err(bad("Scenario operation must target the active root class"));
            }
            classes::field_access(module, function, *class, *field)
                .map_err(|message| invalid(index, Some(pc), &message))?;
        }
        Op::Palantir { class, fields } => {
            let context = function
                .scenario
                .ok_or_else(|| bad("Scenario operation requires a regional frame"))?;
            if *class != context.class {
                return Err(bad("Scenario operation must target the active root class"));
            }
            if fields.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(bad(
                    "Palantir fields must be unique and in declaration order",
                ));
            }
            for field in fields {
                classes::field_access(module, function, *class, *field)
                    .map_err(|message| invalid(index, Some(pc), &message))?;
            }
        }
        _ => {}
    }
    let Some(context) = function.scenario else {
        return Ok(());
    };
    match op {
        Op::Construct(_) | Op::NewObject(_) | Op::IntoRole { .. } | Op::CallRole { .. } => {
            return Err(bad(
                "Allocation and role dispatch are forbidden in scenarios",
            ));
        }
        Op::GetField { class, .. } | Op::SetField { class, .. } if *class != context.class => {
            return Err(bad("Scenario field access requires the active root class"));
        }
        Op::Binary(_, Type::Class(..)) => {
            return Err(bad("Scenario views have no first-class identity"));
        }
        Op::CallIndirect(ty) if !value_callable(ty) => {
            return Err(bad(
                "Scenario indirect calls require value-only signatures without IO",
            ));
        }
        Op::Intrinsic(operation) => {
            let Some(signature) = operation.signature() else {
                return Err(bad("Invalid intrinsic signature"));
            };
            if !value_result(&signature.result)
                || signature.parameters.iter().any(|ty| !ty.is_snapshot())
                || operation.types().iter().any(|ty| !ty.is_snapshot())
                || signature.effects.contains(Effects::IO)
            {
                return Err(bad(
                    "Scenario intrinsics must operate only on snapshot values",
                ));
            }
        }
        Op::Unary(_, ty) | Op::Binary(_, ty) | Op::MatchResult { ty, .. } if !ty.is_snapshot() => {
            return Err(bad("Scenario operands must be snapshot values"));
        }
        _ => {}
    }
    Ok(())
}
