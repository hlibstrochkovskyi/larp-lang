use super::*;
use std::collections::HashSet;

fn constructor_effects() -> Effects {
    Effects::READ.union(Effects::WRITE).union(Effects::ABORT)
}

fn parameters(
    parameters: &[(ast::TypeExpr, Name)],
    class_types: &Scope<Type>,
) -> Result<Vec<Type>> {
    let mut names = HashSet::new();
    parameters
        .iter()
        .map(|(expression, name)| {
            if !names.insert(&name.text) {
                return Err(error("LARP-N002", name.span, "Duplicate parameter name"));
            }
            let result = ty(expression, class_types)?;
            if result == Type::Unit {
                return Err(error(
                    "LARP-T001",
                    name.span,
                    "Function parameters cannot have type unit",
                ));
            }
            Ok(result)
        })
        .collect()
}

pub(super) fn check_program(
    ast: &Declarations<'_>,
    project: Option<&crate::project::Project>,
) -> std::result::Result<Program, Vec<Diagnostic>> {
    let mut body_errors = Vec::new();
    let result = (|| -> Result<Program> {
        if ast.classes.len() > 4096 || ast.roles.len() > 4096 {
            return Err(error(
                "LARP-P003",
                ast.main.span,
                "Class or role count limit exceeded",
            ));
        }
        let mut class_types = Scope::types(project);
        for (index, class) in ast.classes.iter().enumerate() {
            if Type::from_name(&class.name.text).is_some()
                || matches!(
                    class.name.text.as_str(),
                    "mfs" | "hediJeans" | "baddie" | "result"
                )
                || class_types
                    .insert(
                        &class.name,
                        Type::class(index, modules::display_name(&class.name, project.is_some())),
                    )
                    .is_some()
            {
                return Err(error(
                    "LARP-N002",
                    class.name.span,
                    format!("Duplicate or reserved type name '{}'", class.name.text),
                ));
            }
        }
        for (index, role) in ast.roles.iter().enumerate() {
            if Type::from_name(&role.name.text).is_some()
                || matches!(
                    role.name.text.as_str(),
                    "mfs" | "hediJeans" | "baddie" | "result"
                )
                || class_types
                    .insert(
                        &role.name,
                        Type::role(index, modules::display_name(&role.name, project.is_some())),
                    )
                    .is_some()
            {
                return Err(error(
                    "LARP-N002",
                    role.name.span,
                    "Duplicate or reserved type name",
                ));
            }
        }
        let mut roles = Vec::new();
        for role in &ast.roles {
            let mut seen = HashSet::new();
            let mut methods = Vec::new();
            for method in &role.methods {
                if !seen.insert(&method.name.text) {
                    return Err(error(
                        "LARP-N002",
                        method.name.span,
                        "Duplicate role method name",
                    ));
                }
                methods.push(larp_bytecode::RoleMethod {
                    name: method.name.text.clone(),
                    parameters: parameters(&method.parameters, &class_types)?,
                    result: ty(&method.result, &class_types)?,
                    effects: effects(&method.effects)?,
                });
            }
            roles.push(larp_bytecode::Role {
                name: modules::display_name(&role.name, project.is_some()),
                methods,
            });
        }
        let mut classes = Vec::new();
        let mut members = roles.iter().map(|role| role.methods.len()).sum::<usize>();
        if members > 65_536 {
            return Err(error(
                "LARP-P003",
                ast.main.span,
                "Nominal member limit exceeded",
            ));
        }
        for class in &ast.classes {
            if class.constructors.len() != 1 {
                return Err(error(
                    "LARP-T011",
                    class.span,
                    "An archetype must declare exactly one init constructor",
                ));
            }
            members += class.fields.len() + class.methods.len();
            if members > 65_536 {
                return Err(error(
                    "LARP-P003",
                    class.span,
                    "Class member limit exceeded",
                ));
            }
            let mut seen = HashSet::new();
            let mut fields = Vec::new();
            for field in &class.fields {
                if !seen.insert(field.name.text.clone()) {
                    return Err(error(
                        "LARP-N002",
                        field.name.span,
                        "Duplicate class member name",
                    ));
                }
                let field_type = ty(&field.ty, &class_types)?;
                if field_type == Type::Unit {
                    return Err(error(
                        "LARP-T001",
                        field.name.span,
                        "A field cannot have type unit",
                    ));
                }
                fields.push(larp_bytecode::Field {
                    name: field.name.text.clone(),
                    ty: field_type,
                    public: field.public,
                });
            }
            for method in &class.methods {
                if !seen.insert(method.function.name.text.clone()) {
                    return Err(error(
                        "LARP-N002",
                        method.function.name.span,
                        "Fields and methods must have distinct names",
                    ));
                }
            }
            classes.push(larp_bytecode::Class {
                implementations: Vec::new(),
                name: modules::display_name(&class.name, project.is_some()),
                fields,
                methods: Vec::new(),
                constructor: usize::MAX,
            });
        }
        let mut headers = Vec::new();
        let mut bodies = Vec::new();
        let mut parameter_names = Vec::new();
        let mut names = Scope::functions(project);
        let mut extensions = Vec::new();
        let mut extension_names = HashSet::new();
        let public_extensions = modules::exported_extensions(project)?;
        let mut exported_extensions = HashSet::new();
        for function in &ast.functions {
            let mut function_parameters = parameters(&function.parameters, &class_types)?;
            let mut function_names = function
                .parameters
                .iter()
                .map(|(_, name)| name.clone())
                .collect::<Vec<_>>();
            let mut name = function.name.clone();
            name.text = modules::display_name(&function.name, project.is_some());
            if let Some(receiver) = &function.receiver {
                let receiver = ty(receiver, &class_types)?;
                if receiver == Type::Unit {
                    return Err(error(
                        "LARP-T001",
                        function.name.span,
                        "An extension receiver cannot have type unit",
                    ));
                }
                if !extension_names.insert((
                    function.name.span.source(),
                    receiver.clone(),
                    function.name.text.clone(),
                )) {
                    return Err(error(
                        "LARP-N002",
                        function.name.span,
                        "Duplicate extension for this receiver and name",
                    ));
                }
                name.text = format!("{}::{}", receiver, name.text);
                function_parameters.insert(0, receiver.clone());
                function_names.insert(
                    0,
                    Name {
                        text: "self".into(),
                        span: function.name.span,
                    },
                );
                extensions.push(larp_bytecode::Extension {
                    receiver,
                    name: modules::display_name(&function.name, project.is_some()),
                    function: headers.len(),
                });
                if public_extensions.contains(&(
                    function.name.span.source(),
                    function.name.span.range().start,
                )) {
                    exported_extensions.insert(headers.len());
                }
            } else {
                if let Some(previous) = names.get(&function.name).copied() {
                    let mut diagnostic =
                        error("LARP-N002", function.name.span, "Duplicate function name");
                    let previous: usize = previous;
                    diagnostic.related.push(Label {
                        span: ast.functions[previous].name.span,
                        message: "First declaration".into(),
                    });
                    return Err(diagnostic);
                }
                names.insert(&function.name, headers.len());
            }
            headers.push(Header {
                name,
                parameters: function_parameters,
                result: ty(&function.result, &class_types)?,
                effects: effects(&function.effects)?,
                owner: None,
                constructor: None,
                scenario: None,
            });
            bodies.push(&function.body);
            parameter_names.push(function_names);
        }
        for (index, class) in ast.classes.iter().enumerate() {
            let constructor = &class.constructors[0];
            classes[index].constructor = headers.len();
            headers.push(Header {
                name: Name {
                    text: format!(
                        "{}::init",
                        modules::display_name(&class.name, project.is_some())
                    ),
                    span: constructor.span,
                },
                parameters: parameters(&constructor.parameters, &class_types)?,
                result: class_types
                    .get(&class.name)
                    .expect("collected class type")
                    .clone(),
                effects: constructor_effects(),
                owner: Some(index),
                constructor: Some(index),
                scenario: None,
            });
            bodies.push(&constructor.body);
            parameter_names.push(
                constructor
                    .parameters
                    .iter()
                    .map(|(_, name)| name.clone())
                    .collect(),
            );
            for declaration in &class.methods {
                let method = &declaration.function;
                let mut method_parameters = vec![
                    class_types
                        .get(&class.name)
                        .expect("collected class type")
                        .clone(),
                ];
                method_parameters.extend(parameters(&method.parameters, &class_types)?);
                classes[index].methods.push(larp_bytecode::Method {
                    name: method.name.text.clone(),
                    function: headers.len(),
                    public: declaration.public,
                });
                headers.push(Header {
                    name: Name {
                        text: format!(
                            "{}::{}",
                            modules::display_name(&class.name, project.is_some()),
                            method.name.text
                        ),
                        span: method.name.span,
                    },
                    parameters: method_parameters,
                    result: ty(&method.result, &class_types)?,
                    effects: effects(&method.effects)?,
                    owner: Some(index),
                    constructor: None,
                    scenario: None,
                });
                bodies.push(&method.body);
                let mut names = vec![Name {
                    text: "self".into(),
                    span: method.name.span,
                }];
                names.extend(method.parameters.iter().map(|(_, name)| name.clone()));
                parameter_names.push(names);
            }
        }
        for (class_id, class) in ast.classes.iter().enumerate() {
            let mut seen = HashSet::new();
            for name in &class.roles {
                let role = class_types
                    .get(name)
                    .and_then(Type::role_id)
                    .ok_or_else(|| {
                        error(
                            "LARP-T013",
                            name.span,
                            "An archetype may implement only declared roles",
                        )
                    })?;
                if !seen.insert(role) {
                    return Err(error(
                        "LARP-T013",
                        name.span,
                        "Duplicate role implementation",
                    ));
                }
                members += 1 + roles[role].methods.len();
                if members > 65_536 {
                    return Err(error(
                        "LARP-P003",
                        name.span,
                        "Nominal member limit exceeded",
                    ));
                }
                let mut methods = Vec::new();
                for required in &roles[role].methods {
                    let method = classes[class_id]
                        .methods
                        .iter()
                        .find(|method| method.name == required.name)
                        .ok_or_else(|| {
                            error(
                                "LARP-T013",
                                name.span,
                                format!("Missing role method '{}'", required.name),
                            )
                        })?;
                    let target = &headers[method.function];
                    if !method.public
                        || target.parameters[1..] != required.parameters
                        || target.result != required.result
                        || !required.effects.contains(target.effects)
                    {
                        return Err(error(
                            "LARP-T013",
                            target.name.span,
                            "A role implementation must be public, match parameter/result types, and stay within the role effect bound",
                        ));
                    }
                    methods.push(method.function);
                }
                classes[class_id]
                    .implementations
                    .push(larp_bytecode::Implementation { role, methods });
            }
        }
        if headers.len() >= 4096 {
            return Err(error(
                "LARP-P003",
                ast.main.span,
                "Too many functions including methods and constructors",
            ));
        }
        let entry = headers.len();
        headers.push(Header {
            name: Name {
                text: "main_character".into(),
                span: ast.main.span,
            },
            parameters: Vec::new(),
            result: Type::Unit,
            effects: Effects::ALL,
            owner: None,
            constructor: None,
            scenario: None,
        });
        bodies.push(ast.main);
        parameter_names.push(Vec::new());
        for header in &headers {
            if !Type::function(
                header.parameters.clone(),
                header.result.clone(),
                header.effects,
            )
            .within_limits()
            {
                return Err(error(
                    "LARP-P003",
                    header.name.span,
                    "Function signature type complexity limit exceeded",
                ));
            }
        }
        for (role, declaration) in roles.iter().zip(&ast.roles) {
            for (method, declaration) in role.methods.iter().zip(&declaration.methods) {
                if !Type::function(
                    method.parameters.clone(),
                    method.result.clone(),
                    method.effects,
                )
                .within_limits()
                {
                    return Err(error(
                        "LARP-P003",
                        declaration.name.span,
                        "Role signature type complexity limit exceeded",
                    ));
                }
            }
        }
        if let Some(project) = project {
            modules::validate_exports(project, ast, &class_types)?;
        }
        let tables = Tables {
            project: project.is_some(),
            headers,
            names,
            types: class_types,
            classes,
            roles,
            extensions,
            exported_extensions,
        };
        let mut functions = Vec::new();
        let mut generated = Vec::new();
        let mut errors = Vec::new();
        for (index, header) in tables.headers.iter().enumerate() {
            let mut builder = Builder::new(
                header,
                &tables,
                bodies[index].span,
                tables.headers.len() + generated.len(),
            );
            for (slot, name) in parameter_names[index].iter().enumerate() {
                if let Err(error) = builder.declare(
                    name,
                    Binding {
                        lazy: None,
                        slot,
                        ty: header.parameters[slot].clone(),
                        writable: name.text != "self",
                        span: name.span,
                    },
                ) {
                    builder.errors.push(*error);
                }
            }
            if let Some(class) = header.constructor {
                for (field, declaration) in tables.classes[class]
                    .fields
                    .iter()
                    .zip(&ast.classes[class].fields)
                {
                    let slot = builder.local(field.ty.clone());
                    builder.constructor_fields.push(Binding {
                        lazy: None,
                        slot,
                        ty: field.ty.clone(),
                        writable: true,
                        span: declaration.name.span,
                    });
                }
            }
            builder.block(bodies[index], false);
            builder.finish();
            if builder.function.constructor.is_some() && builder.errors.is_empty() {
                builder.check_initialization();
            }
            generated.extend(builder.generated);
            functions.push(builder.function);
            errors.extend(builder.errors);
        }
        if !errors.is_empty() {
            let first = errors[0].clone();
            body_errors = errors;
            return Err(Box::new(first));
        }
        functions.extend(generated);
        let mut program = Program {
            classes: tables.classes,
            roles: tables.roles,
            extensions: tables.extensions,
            functions,
            entry,
        };
        scenarios::specialize(&mut program)?;
        Ok(program)
    })();
    result.map_err(|error| {
        if body_errors.is_empty() {
            vec![*error]
        } else {
            body_errors
        }
    })
}
