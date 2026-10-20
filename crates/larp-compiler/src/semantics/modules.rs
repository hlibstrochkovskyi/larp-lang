use super::*;
use crate::project::Project;
use larp_source::SourceId;
use std::collections::HashSet;

type Symbol = (SourceId, String);

/// Lookups retain the lexical file identity. Import aliases do not create a
/// Cartesian product of copied declaration maps.
pub(super) struct Scope<T> {
    symbols: HashMap<Symbol, T>,
    imports: HashMap<Symbol, SourceId>,
    public: HashSet<Symbol>,
}

impl<T> Scope<T> {
    fn new(project: Option<&Project>, types: bool) -> Self {
        let mut scope = Self {
            symbols: HashMap::new(),
            imports: HashMap::new(),
            public: HashSet::new(),
        };
        if let Some(project) = project {
            for module in project.modules() {
                for import in &module.imports {
                    let target = project
                        .module(import.target)
                        .expect("validated import edge");
                    scope
                        .imports
                        .insert((module.source, import.alias.text.clone()), target.source);
                }
                if types {
                    for item in &module.unit.classes {
                        if item.public {
                            scope
                                .public
                                .insert((module.source, item.item.name.text.clone()));
                        }
                    }
                    for item in &module.unit.roles {
                        if item.public {
                            scope
                                .public
                                .insert((module.source, item.item.name.text.clone()));
                        }
                    }
                } else {
                    for item in &module.unit.functions {
                        if item.public && item.item.receiver.is_none() {
                            scope
                                .public
                                .insert((module.source, item.item.name.text.clone()));
                        }
                    }
                }
            }
        }
        scope
    }

    pub(super) fn types(project: Option<&Project>) -> Self {
        Self::new(project, true)
    }
    pub(super) fn functions(project: Option<&Project>) -> Self {
        Self::new(project, false)
    }

    pub(super) fn insert(&mut self, name: &Name, value: T) -> Option<T> {
        self.symbols
            .insert((name.span.source(), name.text.clone()), value)
    }

    pub(super) fn get(&self, name: &Name) -> Option<&T> {
        let source = name.span.source();
        if let Some((alias, item)) = name.text.split_once("::") {
            if item.contains("::") {
                return None;
            }
            let target = *self.imports.get(&(source, alias.to_owned()))?;
            let key = (target, item.to_owned());
            if !self.public.contains(&key) {
                return None;
            }
            self.symbols.get(&key)
        } else {
            self.symbols.get(&(source, name.text.clone()))
        }
    }

    fn qualified_target<'a>(&self, name: &'a Name) -> Option<(SourceId, &'a str)> {
        let (alias, item) = name.text.split_once("::")?;
        if item.contains("::") {
            return None;
        }
        let target = *self.imports.get(&(name.span.source(), alias.to_owned()))?;
        Some((target, item))
    }
}

impl Tables {
    pub(super) fn extension_candidates(&self, name: &Name) -> Vec<usize> {
        let Some((source, item)) = self.names.qualified_target(name) else {
            return Vec::new();
        };
        let qualified = format!("module{}::{item}", source.0);
        self.extensions
            .iter()
            .filter(|extension| {
                extension.name == qualified
                    && self.exported_extensions.contains(&extension.function)
            })
            .map(|extension| extension.function)
            .collect()
    }

    pub(super) fn function_value(
        &self,
        name: &Name,
        expected: Option<&Type>,
    ) -> Result<Option<usize>> {
        if let Some(index) = self.names.get(name) {
            return Ok(Some(*index));
        }
        let candidates = self.extension_candidates(name);
        match candidates.as_slice() {
            [] => Ok(None),
            [index] => Ok(Some(*index)),
            _ => {
                let compatible = candidates
                    .into_iter()
                    .filter(|index| {
                        let header = &self.headers[*index];
                        expected.is_some_and(|expected| {
                            expected.accepts(&Type::function(
                                header.parameters.clone(),
                                header.result.clone(),
                                header.effects,
                            ))
                        })
                    })
                    .collect::<Vec<_>>();
                if let [index] = compatible.as_slice() {
                    Ok(Some(*index))
                } else {
                    Err(error(
                        "LARP-M007",
                        name.span,
                        "An exported extension name with multiple receivers needs an unambiguous function type",
                    ))
                }
            }
        }
    }
}

pub(super) fn exported_extensions(project: Option<&Project>) -> Result<HashSet<(SourceId, usize)>> {
    let mut extensions = HashSet::new();
    if let Some(project) = project {
        for module in project.modules() {
            let mut names: HashMap<&str, (&Name, bool)> = HashMap::new();
            for declaration in &module.unit.functions {
                if !declaration.public {
                    continue;
                }
                let function = &declaration.item;
                let extension = function.receiver.is_some();
                if let Some((previous, was_extension)) = names.get(function.name.text.as_str()) {
                    if extension != *was_extension {
                        let mut diagnostic = error(
                            "LARP-M007",
                            function.name.span,
                            "An exported function and exported extension cannot share a name in one module",
                        );
                        diagnostic.related.push(Label {
                            span: previous.span,
                            message: "First exported declaration".into(),
                        });
                        return Err(diagnostic);
                    }
                } else {
                    names.insert(&function.name.text, (&function.name, extension));
                }
                if extension {
                    extensions.insert((
                        function.name.span.source(),
                        function.name.span.range().start,
                    ));
                }
            }
        }
    }
    Ok(extensions)
}

pub(super) fn display_name(name: &Name, project: bool) -> String {
    if project {
        format!("module{}::{}", name.span.source().0, name.text)
    } else {
        name.text.clone()
    }
}

/// Borrowed declaration inventory: the original ASTs, names and source spans
/// remain intact throughout whole-project checking.
pub(super) struct Declarations<'a> {
    pub roles: Vec<&'a ast::Role>,
    pub classes: Vec<&'a ast::Class>,
    pub functions: Vec<&'a ast::Function>,
    pub main: &'a ast::Block,
}

impl<'a> Declarations<'a> {
    pub(super) fn single(program: &'a ast::Program) -> Self {
        Self {
            roles: program.roles.iter().collect(),
            classes: program.classes.iter().collect(),
            functions: program.functions.iter().collect(),
            main: &program.main,
        }
    }

    pub(super) fn project(project: &'a Project) -> Self {
        Self {
            roles: project
                .modules()
                .iter()
                .flat_map(|m| m.unit.roles.iter().map(|d| &d.item))
                .collect(),
            classes: project
                .modules()
                .iter()
                .flat_map(|m| m.unit.classes.iter().map(|d| &d.item))
                .collect(),
            functions: project
                .modules()
                .iter()
                .flat_map(|m| m.unit.functions.iter().map(|d| &d.item))
                .collect(),
            main: project
                .module(project.entry())
                .expect("validated entry")
                .unit
                .main
                .as_ref()
                .expect("validated entry block"),
        }
    }
}

pub(super) fn check(project: &Project) -> std::result::Result<Program, Vec<Diagnostic>> {
    declarations::check_program(&Declarations::project(project), Some(project))
}

/// A private nominal type must not escape through an exported signature,
/// including when its spelling is hidden by inference at the call site.
pub(super) fn validate_exports(
    project: &Project,
    declarations: &Declarations<'_>,
    types: &Scope<Type>,
) -> Result<()> {
    fn visible(
        ty: &Type,
        span: Span,
        declarations: &Declarations<'_>,
        types: &Scope<Type>,
    ) -> Result<()> {
        let mut pending = vec![ty];
        while let Some(ty) = pending.pop() {
            let declaration = match ty {
                Type::Class(id, _) => Some(&declarations.classes[*id].name),
                Type::Role(id, _) => Some(&declarations.roles[*id].name),
                Type::Baddie(value)
                | Type::Array(value)
                | Type::FixedArray(value, _)
                | Type::Lazy(value, _) => {
                    pending.push(value);
                    None
                }
                Type::Result(pair) => {
                    pending.push(&pair.0);
                    pending.push(&pair.1);
                    None
                }
                Type::Function(signature) => {
                    pending.extend(&signature.parameters);
                    pending.push(&signature.result);
                    None
                }
                _ => None,
            };
            if let Some(name) = declaration
                && !types
                    .public
                    .contains(&(name.span.source(), name.text.clone()))
            {
                let mut diagnostic = error(
                    "LARP-M006",
                    span,
                    format!(
                        "Exported interface exposes module-private type '{}'",
                        name.text
                    ),
                );
                diagnostic.related.push(Label {
                    span: name.span,
                    message: "Private type declared here".into(),
                });
                return Err(diagnostic);
            }
        }
        Ok(())
    }
    let check = |expression: &ast::TypeExpr| -> Result<()> {
        visible(
            &ty(expression, types)?,
            expression.span,
            declarations,
            types,
        )
    };
    let function = |function: &ast::Function| -> Result<()> {
        if let Some(receiver) = &function.receiver {
            check(receiver)?;
        }
        for (parameter, _) in &function.parameters {
            check(parameter)?;
        }
        check(&function.result)
    };
    for module in project.modules() {
        for item in &module.unit.functions {
            if item.public {
                function(&item.item)?;
            }
        }
        for item in &module.unit.roles {
            if !item.public {
                continue;
            }
            for method in &item.item.methods {
                for (parameter, _) in &method.parameters {
                    check(parameter)?;
                }
                check(&method.result)?;
            }
        }
        for item in &module.unit.classes {
            if !item.public {
                continue;
            }
            for role in &item.item.roles {
                if let Some(ty) = types.get(role) {
                    visible(ty, role.span, declarations, types)?;
                }
            }
            for constructor in &item.item.constructors {
                for (parameter, _) in &constructor.parameters {
                    check(parameter)?;
                }
            }
            for field in &item.item.fields {
                if field.public {
                    check(&field.ty)?;
                }
            }
            for method in &item.item.methods {
                if method.public {
                    function(&method.function)?;
                }
            }
        }
    }
    Ok(())
}
