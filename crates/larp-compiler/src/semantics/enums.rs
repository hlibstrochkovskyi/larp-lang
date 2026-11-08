use super::*;
use larp_types::{EnumType, EnumVariant, MAX_ENUM_PAYLOAD, MAX_ENUM_VARIANTS};
use std::collections::{BTreeSet, HashSet, VecDeque};
use std::sync::Arc;

/// Resolve value layouts in dependency order while preserving source-order IDs.
pub(super) fn collect(
    declarations: &Declarations<'_>,
    project: Option<&crate::project::Project>,
    types: &mut Scope<Type>,
) -> Result<Vec<Arc<EnumType>>> {
    if declarations.enums.len() > 4096 {
        return Err(error(
            "LARP-P003",
            declarations.main.span,
            "Enum count limit exceeded",
        ));
    }
    let mut names = Scope::types(project);
    for (id, declaration) in declarations.enums.iter().enumerate() {
        let name = &declaration.name;
        if Type::from_name(&name.text).is_some()
            || matches!(
                name.text.as_str(),
                "mfs" | "hediJeans" | "baddie" | "result"
            )
            || types.get(name).is_some()
            || types.is_alias(name)
            || names.insert(name, id).is_some()
        {
            return Err(error(
                "LARP-T015",
                name.span,
                "Enum name duplicates a type, reserved type, or import alias",
            ));
        }
        if declaration.variants.is_empty() || declaration.variants.len() > MAX_ENUM_VARIANTS {
            return Err(error(
                "LARP-T015",
                declaration.span,
                "An enum requires 1 to 256 variants",
            ));
        }
        let mut variants = HashSet::new();
        for variant in &declaration.variants {
            if !variants.insert(&variant.name.text) {
                return Err(error(
                    "LARP-T015",
                    variant.name.span,
                    "Duplicate enum variant name",
                ));
            }
            if variant.payloads.len() > MAX_ENUM_PAYLOAD {
                return Err(error(
                    "LARP-T015",
                    variant.span,
                    "An enum variant permits at most 64 payload fields",
                ));
            }
        }
    }
    fn dependencies(expression: &ast::TypeExpr, names: &Scope<usize>, found: &mut BTreeSet<usize>) {
        match &expression.kind {
            ast::TypeExprKind::Named { name, arguments } => {
                if let Some(id) = names.get(name) {
                    found.insert(*id);
                }
                for argument in arguments {
                    dependencies(argument, names, found);
                }
            }
            ast::TypeExprKind::Function {
                parameters, result, ..
            } => {
                for parameter in parameters {
                    dependencies(parameter, names, found);
                }
                dependencies(result, names, found);
            }
            ast::TypeExprKind::Integer(_) => {}
        }
    }
    let mut counts = vec![0; declarations.enums.len()];
    let mut users = vec![Vec::new(); declarations.enums.len()];
    let mut members = 0usize;
    for (id, declaration) in declarations.enums.iter().enumerate() {
        let mut found = BTreeSet::new();
        for variant in &declaration.variants {
            members += 1 + variant.payloads.len();
            if members > 65_536 {
                return Err(error(
                    "LARP-P003",
                    variant.span,
                    "Enum member limit exceeded",
                ));
            }
            for payload in &variant.payloads {
                dependencies(payload, &names, &mut found);
            }
        }
        counts[id] = found.len();
        for dependency in found {
            users[dependency].push(id);
        }
    }
    let mut pending = counts
        .iter()
        .enumerate()
        .filter_map(|(id, count)| (*count == 0).then_some(id))
        .collect::<VecDeque<_>>();
    let mut layouts = vec![None; declarations.enums.len()];
    while let Some(id) = pending.pop_front() {
        let declaration = declarations.enums[id];
        let mut variants = Vec::new();
        for variant in &declaration.variants {
            let mut payload = Vec::new();
            for expression in &variant.payloads {
                let field = ty(expression, types)?;
                if field == Type::Unit {
                    return Err(error(
                        "LARP-T015",
                        expression.span,
                        "An enum payload cannot have type unit",
                    ));
                }
                payload.push(field);
            }
            variants.push(EnumVariant {
                name: Arc::from(variant.name.text.as_str()),
                payload,
            });
        }
        let layout = Arc::new(EnumType {
            id,
            name: Arc::from(modules::display_name(&declaration.name, project.is_some())),
            variants,
        });
        let value_type = Type::Enum(layout.clone());
        if !value_type.within_limits() {
            return Err(error(
                "LARP-P003",
                declaration.span,
                "Expanded enum type exceeds complexity limits",
            ));
        }
        if !value_type.is_well_formed() {
            return Err(error(
                "LARP-T015",
                declaration.span,
                "Invalid enum payload layout",
            ));
        }
        types.insert(&declaration.name, value_type);
        layouts[id] = Some(layout);
        for &user in &users[id] {
            counts[user] -= 1;
            if counts[user] == 0 {
                pending.push_back(user);
            }
        }
    }
    if let Some(id) = layouts.iter().position(Option::is_none) {
        return Err(error(
            "LARP-T015",
            declarations.enums[id].span,
            "Recursive enum payload layouts are not supported",
        ));
    }
    Ok(layouts.into_iter().map(Option::unwrap).collect())
}

impl Tables {
    pub(super) fn enum_variant(&self, path: &Name) -> Result<Option<(Type, usize)>> {
        let Some((prefix, variant)) = path.text.rsplit_once("::") else {
            return Ok(None);
        };
        let name = Name {
            text: prefix.to_owned(),
            span: path.span,
        };
        let Some(ty @ Type::Enum(layout)) = self.types.get(&name) else {
            return Ok(None);
        };
        let index = layout
            .variants
            .iter()
            .position(|item| item.name.as_ref() == variant)
            .ok_or_else(|| {
                error(
                    "LARP-T015",
                    path.span,
                    format!("Unknown variant '{variant}' of {ty}"),
                )
            })?;
        Ok(Some((ty.clone(), index)))
    }
}

impl Builder<'_> {
    pub(super) fn construct_enum(
        &mut self,
        ty: Type,
        variant: usize,
        arguments: &[Expr],
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        let Type::Enum(layout) = &ty else {
            unreachable!("resolved enum constructor");
        };
        let payload = layout.variants[variant].payload.clone();
        if payload.is_empty() {
            return Err(error(
                "LARP-T015",
                span,
                "A nullary enum variant is a value; omit parentheses",
            ));
        }
        if arguments.len() != payload.len() {
            return Err(error(
                "LARP-T015",
                span,
                format!(
                    "Variant requires {} payloads, found {}",
                    payload.len(),
                    arguments.len()
                ),
            ));
        }
        let mut values = Vec::new();
        for (expression, expected) in arguments.iter().zip(payload) {
            values.push(self.expression(expression, Some(expected), evaluate)?.slot);
        }
        Ok(self.enum_value(ty, variant, values, span))
    }

    fn enum_value(&mut self, ty: Type, variant: usize, arguments: Vec<Local>, span: Span) -> Value {
        let slot = self.local(ty.clone());
        self.emit(
            Inst::MakeEnum {
                destination: slot,
                ty: ty.clone(),
                variant,
                arguments,
            },
            span,
        );
        Value {
            slot,
            ty,
            constant: None,
        }
    }

    pub(super) fn enum_reference(&mut self, ty: Type, variant: usize, span: Span) -> Result<Value> {
        let Type::Enum(layout) = &ty else {
            unreachable!("resolved enum constructor");
        };
        let parameters = layout.variants[variant].payload.clone();
        if parameters.is_empty() {
            return Ok(self.enum_value(ty, variant, Vec::new(), span));
        }
        let function_type = Type::function(parameters.clone(), ty.clone(), Effects::NONE);
        if !function_type.within_limits() {
            return Err(error(
                "LARP-P003",
                span,
                "Enum constructor function type exceeds complexity limits",
            ));
        }
        let id = self.generated_base + self.generated.len();
        if id >= 4096 {
            return Err(error(
                "LARP-P003",
                span,
                "Generated function limit exceeded",
            ));
        }
        let header = Header {
            name: Name {
                text: format!("{}::{}@constructor", ty, layout.variants[variant].name),
                span,
            },
            parameters,
            result: ty.clone(),
            effects: Effects::NONE,
            owner: None,
            constructor: None,
            scenario: self.function.scenario,
        };
        let mut child = Builder::new(&header, self.tables, span, id);
        let value = child.enum_value(ty, variant, (0..header.parameters.len()).collect(), span);
        child.terminate(Terminator::Return(Some(value.slot)), span);
        self.generated.push(child.function);
        let slot = self.local(function_type.clone());
        self.emit(
            Inst::Closure {
                destination: slot,
                function: id,
                captures: Vec::new(),
            },
            span,
        );
        Ok(Value {
            slot,
            ty: function_type,
            constant: None,
        })
    }
}
