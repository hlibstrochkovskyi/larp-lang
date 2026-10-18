//! Nominal layout and access checks performed before bytecode dataflow.

use crate::{Function, Module, Op, VerifyError, VerifyErrorKind};
use larp_types::{Effects, Type};
use std::collections::HashSet;

fn invalid(function: usize, instruction: Option<usize>, message: impl Into<String>) -> VerifyError {
    VerifyError {
        kind: VerifyErrorKind::Invalid,
        function,
        instruction,
        message: message.into(),
    }
}

pub(super) fn validate_type(
    module: &Module,
    ty: &Type,
    function: usize,
    instruction: Option<usize>,
) -> Result<(), VerifyError> {
    if !ty.within_limits() {
        return Err(invalid(function, instruction, "Type complexity limit exceeded").resource());
    }
    if !ty.is_well_formed() {
        return Err(invalid(function, instruction, "Malformed type"));
    }
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        match ty {
            Type::Role(id, name) => {
                if module
                    .roles
                    .get(*id)
                    .is_none_or(|role| role.name.as_str() != name.as_ref())
                {
                    return Err(invalid(
                        function,
                        instruction,
                        "Invalid nominal role identity or name",
                    ));
                }
            }
            Type::Class(id, name) => {
                if module
                    .classes
                    .get(*id)
                    .is_none_or(|class| class.name.as_str() != name.as_ref())
                {
                    return Err(invalid(
                        function,
                        instruction,
                        "Invalid nominal class identity or name",
                    ));
                }
            }
            Type::Array(ty) | Type::FixedArray(ty, _) | Type::Baddie(ty) => pending.push(ty),
            Type::Result(types) => {
                pending.push(&types.0);
                pending.push(&types.1);
            }
            Type::Function(signature) => {
                pending.extend(&signature.parameters);
                pending.push(&signature.result);
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn preflight_types(module: &Module) -> Result<(), VerifyError> {
    if module.roles.len() > 4096 || module.extensions.len() > 4096 {
        return Err(
            invalid(module.entry, None, "Role or extension count limit exceeded").resource(),
        );
    }
    if module.classes.len() > 4096 {
        return Err(invalid(module.entry, None, "Class count limit exceeded").resource());
    }
    let mut members = 0usize;
    let mut names = HashSet::new();
    for role in &module.roles {
        if role.name.is_empty() || !names.insert(role.name.as_str()) {
            return Err(invalid(
                module.entry,
                None,
                "Duplicate or empty nominal name",
            ));
        }
        members = members.saturating_add(role.methods.len());
        if members > 65_536 {
            return Err(invalid(module.entry, None, "Role member limit exceeded").resource());
        }
        for method in &role.methods {
            if method.parameters.len() > 4096 {
                return Err(invalid(module.entry, None, "Role parameter limit exceeded").resource());
            }
            // Validate the complete signature before any structural comparison.
            validate_type(
                module,
                &Type::function(
                    method.parameters.clone(),
                    method.result.clone(),
                    method.effects,
                ),
                module.entry,
                None,
            )?;
        }
    }
    for extension in &module.extensions {
        validate_type(module, &extension.receiver, extension.function, None)?;
    }
    for class in &module.classes {
        members = members
            .saturating_add(class.fields.len())
            .saturating_add(class.methods.len());
        for implementation in &class.implementations {
            members = members
                .saturating_add(1)
                .saturating_add(implementation.methods.len());
        }
        if members > 65_536 {
            return Err(invalid(module.entry, None, "Class member limit exceeded").resource());
        }
        if class.name.is_empty() || !names.insert(class.name.as_str()) {
            return Err(invalid(module.entry, None, "Duplicate or empty class name"));
        }
        for field in &class.fields {
            validate_type(module, &field.ty, class.constructor, None)?;
            if field.ty == Type::Unit {
                return Err(invalid(
                    class.constructor,
                    None,
                    "Class fields cannot have unit type",
                ));
            }
        }
    }
    for (index, function) in module.functions.iter().enumerate() {
        for ty in function
            .parameters
            .iter()
            .chain(&function.captures)
            .chain(&function.locals)
            .chain(std::iter::once(&function.result))
        {
            validate_type(module, ty, index, None)?;
        }
        for (pc, instruction) in function.code.iter().enumerate() {
            match &instruction.op {
                Op::Unary(_, ty)
                | Op::Binary(_, ty)
                | Op::CallIndirect(ty)
                | Op::MatchResult { ty, .. } => validate_type(module, ty, index, Some(pc))?,
                Op::Intrinsic(operation) => {
                    for ty in operation.types() {
                        validate_type(module, ty, index, Some(pc))?;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub(super) fn validate_layouts(module: &Module) -> Result<(), VerifyError> {
    let constructor_effects = Effects::READ.union(Effects::WRITE).union(Effects::ABORT);
    for (index, function) in module.functions.iter().enumerate() {
        if function
            .owner
            .is_some_and(|class| class >= module.classes.len())
        {
            return Err(invalid(index, None, "Function owner class out of bounds"));
        }
        if let Some(class) = function.constructor
            && (function.owner != Some(class)
                || module
                    .classes
                    .get(class)
                    .is_none_or(|layout| layout.constructor != index))
        {
            return Err(invalid(
                index,
                None,
                "Constructor identity does not match its class",
            ));
        }
    }
    let mut method_targets = HashSet::new();
    for (index, class) in module.classes.iter().enumerate() {
        let Some(constructor) = module.functions.get(class.constructor) else {
            return Err(invalid(
                module.entry,
                None,
                "Constructor function out of bounds",
            ));
        };
        let class_type = Type::class(index, class.name.as_str());
        if constructor.owner != Some(index)
            || constructor.constructor != Some(index)
            || !constructor.captures.is_empty()
            || constructor.result != class_type
            || constructor.effects != constructor_effects
        {
            return Err(invalid(
                class.constructor,
                None,
                "Invalid constructor signature or effects",
            ));
        }
        let mut members = HashSet::new();
        for field in &class.fields {
            if field.name.is_empty() || !members.insert(field.name.as_str()) {
                return Err(invalid(
                    class.constructor,
                    None,
                    "Duplicate or empty class member name",
                ));
            }
        }
        for method in &class.methods {
            if method.name.is_empty() || !members.insert(method.name.as_str()) {
                return Err(invalid(
                    class.constructor,
                    None,
                    "Duplicate or empty class member name",
                ));
            }
            let Some(function) = module.functions.get(method.function) else {
                return Err(invalid(
                    class.constructor,
                    None,
                    "Method function out of bounds",
                ));
            };
            if !method_targets.insert(method.function)
                || function.owner != Some(index)
                || function.constructor.is_some()
                || !function.captures.is_empty()
                || function.parameters.first() != Some(&class_type)
            {
                return Err(invalid(
                    method.function,
                    None,
                    "Invalid method receiver or ownership",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn call_access(module: &Module, caller: &Function, target: usize) -> Result<(), String> {
    let callee = &module.functions[target];
    if callee.constructor.is_some() {
        return Err("Constructors require the Construct instruction".into());
    }
    if let Some(class) = callee.owner
        && caller.owner != Some(class)
        && !module.classes[class]
            .methods
            .iter()
            .any(|method| method.function == target && method.public)
    {
        return Err("Private method or lexical helper is inaccessible".into());
    }
    Ok(())
}

pub(super) fn field_access(
    module: &Module,
    caller: &Function,
    class: usize,
    field: usize,
) -> Result<(), String> {
    let layout = module
        .classes
        .get(class)
        .ok_or("Class index out of bounds")?;
    let field = layout
        .fields
        .get(field)
        .ok_or("Field index out of bounds")?;
    if !field.public && caller.owner != Some(class) {
        return Err("Private field is inaccessible".into());
    }
    Ok(())
}
