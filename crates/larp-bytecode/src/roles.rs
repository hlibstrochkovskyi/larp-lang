//! Role implementation and concrete extension contracts for verified dispatch.
use crate::{Module, VerifyError, VerifyErrorKind};
use std::collections::HashSet;

pub(super) fn validate_layouts(module: &Module) -> Result<(), VerifyError> {
    let invalid = |function, message: &str| VerifyError {
        kind: VerifyErrorKind::Invalid,
        function,
        instruction: None,
        message: message.into(),
    };
    for role in &module.roles {
        let mut names = HashSet::new();
        for method in &role.methods {
            if method.name.is_empty() || !names.insert(&method.name) {
                return Err(invalid(module.entry, "Duplicate or empty role method name"));
            }
        }
    }
    for class in &module.classes {
        let public_methods: HashSet<_> = class
            .methods
            .iter()
            .filter(|method| method.public)
            .map(|method| (method.function, method.name.as_str()))
            .collect();
        let mut implemented = HashSet::new();
        for implementation in &class.implementations {
            let role = module.roles.get(implementation.role).ok_or_else(|| {
                invalid(class.constructor, "Role implementation index out of bounds")
            })?;
            if !implemented.insert(implementation.role)
                || implementation.methods.len() != role.methods.len()
            {
                return Err(invalid(
                    class.constructor,
                    "Duplicate or incomplete role implementation",
                ));
            }
            for (target, signature) in implementation.methods.iter().zip(&role.methods) {
                if !public_methods.contains(&(*target, signature.name.as_str())) {
                    return Err(invalid(
                        class.constructor,
                        "Role target must be a public instance method of the same name",
                    ));
                }
                let function = &module.functions[*target];
                if function.parameters[1..] != signature.parameters
                    || function.result != signature.result
                    || !signature.effects.contains(function.effects)
                {
                    return Err(invalid(
                        *target,
                        "Role implementation signature or effects disagree",
                    ));
                }
            }
        }
    }
    let mut names = HashSet::new();
    let mut functions = HashSet::new();
    for extension in &module.extensions {
        if extension.name.is_empty()
            || !names.insert((&extension.receiver, &extension.name))
            || !functions.insert(extension.function)
        {
            return Err(invalid(
                extension.function,
                "Duplicate or empty extension declaration",
            ));
        }
        let function = module
            .functions
            .get(extension.function)
            .ok_or_else(|| invalid(module.entry, "Extension function index out of bounds"))?;
        if function.owner.is_some()
            || function.constructor.is_some()
            || !function.captures.is_empty()
            || function.parameters.first() != Some(&extension.receiver)
        {
            return Err(invalid(
                extension.function,
                "Invalid extension receiver or ownership",
            ));
        }
    }
    Ok(())
}
