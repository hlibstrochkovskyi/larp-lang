use super::*;
use std::collections::HashSet;

struct ArmPlan {
    block: BlockId,
    bindings: Vec<(Name, Type, Local)>,
}

impl Builder<'_> {
    fn pattern_alternative(
        &self,
        pattern: &ast::Pattern,
        scrutinee: &Type,
    ) -> Result<Option<(usize, Vec<Name>)>> {
        use ast::PatternKind;
        let invalid = |message| error("LARP-T016", pattern.span, message);
        let (index, bindings) = match &pattern.kind {
            PatternKind::Wildcard => return Ok(None),
            PatternKind::Verdict(value) if *scrutinee == Type::Verdict => {
                (usize::from(*value), Vec::new())
            }
            PatternKind::None if matches!(scrutinee, Type::Baddie(_)) => (0, Vec::new()),
            PatternKind::Constructor { path, bindings } => {
                let builtin = match (scrutinee, path.text.as_str()) {
                    (Type::Baddie(_), "some") => Some(1),
                    (Type::Result(_), "win") => Some(0),
                    (Type::Result(_), "fumble") => Some(1),
                    _ => None,
                };
                if let Some(index) = builtin {
                    let bindings = bindings.as_ref().ok_or_else(|| {
                        invalid("A payload pattern requires parentheses and its binders")
                    })?;
                    if bindings.len() != 1 {
                        return Err(invalid(
                            "This built-in variant requires exactly one payload binder",
                        ));
                    }
                    (index, bindings.clone())
                } else {
                    let Some((ty, index)) = self.tables.enum_variant(path)? else {
                        return Err(invalid(
                            "Pattern does not name an alternative of the scrutinee type",
                        ));
                    };
                    if ty != *scrutinee {
                        return Err(invalid(
                            "Enum pattern must name the scrutinee's exact nominal enum type",
                        ));
                    }
                    let Type::Enum(layout) = ty else {
                        unreachable!("resolved enum pattern")
                    };
                    let count = layout.variants[index].payload.len();
                    if count == 0 {
                        if bindings.is_some() {
                            return Err(invalid("A nullary enum pattern uses no parentheses"));
                        }
                        (index, Vec::new())
                    } else {
                        let bindings = bindings
                            .as_ref()
                            .ok_or_else(|| invalid("An enum payload pattern requires binders"))?;
                        if bindings.len() != count {
                            return Err(invalid(
                                "Enum pattern binder count does not match its variant",
                            ));
                        }
                        (index, bindings.clone())
                    }
                }
            }
            _ => return Err(invalid("Pattern does not match this finite sum type")),
        };
        let mut names = HashSet::new();
        for name in &bindings {
            if name.text != "_" && !names.insert(&name.text) {
                return Err(error(
                    "LARP-T016",
                    name.span,
                    "Duplicate payload binder in one match arm",
                ));
            }
        }
        Ok(Some((index, bindings)))
    }

    pub(super) fn match_expression(
        &mut self,
        expression: &Expr,
        arms: &[ast::MatchArm],
        expected: Option<Type>,
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        if arms.is_empty() || arms.len() > 257 {
            return Err(error("LARP-T016", span, "A match requires 1 to 257 arms"));
        }
        let value = self.expression(expression, None, evaluate)?;
        let alternatives = value.ty.alternatives().ok_or_else(|| {
            error(
                "LARP-T016",
                expression.span,
                "match requires verdict, baddie, result, or an enum",
            )
        })?;
        let mut covered = vec![None; alternatives.len()];
        let mut selections = Vec::new();
        for arm in arms {
            let alternative = self.pattern_alternative(&arm.pattern, &value.ty)?;
            let selected = if let Some((index, bindings)) = alternative {
                if let Some(previous) = covered[index] {
                    let mut diagnostic = error(
                        "LARP-T016",
                        arm.pattern.span,
                        "Duplicate or unreachable match alternative",
                    );
                    diagnostic.related.push(Label {
                        span: previous,
                        message: "Alternative already covered here".into(),
                    });
                    return Err(diagnostic);
                }
                vec![(index, bindings)]
            } else {
                let selected = covered
                    .iter()
                    .enumerate()
                    .filter_map(|(index, previous)| {
                        previous.is_none().then_some((index, Vec::new()))
                    })
                    .collect::<Vec<_>>();
                if selected.is_empty() {
                    return Err(error(
                        "LARP-T016",
                        arm.pattern.span,
                        "Match arm follows complete coverage",
                    ));
                }
                selected
            };
            for (index, _) in &selected {
                covered[*index] = Some(arm.pattern.span);
            }
            selections.push(selected);
        }
        if covered.iter().any(Option::is_none) {
            return Err(error(
                "LARP-T016",
                span,
                "Nonexhaustive match; cover every alternative or add a final _ arm",
            ));
        }
        let dispatch = self.current;
        let done = self.new_block(span);
        let mut targets = vec![0; alternatives.len()];
        let mut plans = Vec::new();
        // Connect all incoming arm edges before body checking. Lambda return
        // inference and constructor initialization must see genuine CFG paths.
        for (arm, selected) in arms.iter().zip(selections) {
            let body = self.new_block(arm.span);
            let mut body_bindings = Vec::new();
            for (index, binders) in selected {
                let adapter = self.new_block(arm.pattern.span);
                let mut parameters = Vec::new();
                for (position, payload) in alternatives[index].iter().enumerate() {
                    let slot = self.local(payload.clone());
                    parameters.push(slot);
                    if let Some(name) = binders.get(position)
                        && name.text != "_"
                    {
                        body_bindings.push((name.clone(), payload.clone(), slot));
                    }
                }
                self.function.blocks[adapter].parameters = parameters;
                self.function.blocks[adapter].terminator = Terminator::Goto(body);
                targets[index] = adapter;
            }
            plans.push(ArmPlan {
                block: body,
                bindings: body_bindings,
            });
        }
        self.current = dispatch;
        if let Some(Scalar::Verdict(selected)) = value.constant {
            self.terminate(Terminator::Goto(targets[usize::from(selected)]), span);
        } else {
            self.terminate(
                Terminator::MatchSum {
                    value: value.slot,
                    targets,
                },
                span,
            );
        }
        let mut result_type = expected.clone();
        let mut completed = Vec::new();
        for (arm, plan) in arms.iter().zip(plans) {
            self.current = plan.block;
            self.scopes.push(HashMap::new());
            let checked = (|| -> Result<(Value, bool)> {
                for (name, ty, slot) in plan.bindings {
                    self.declare(
                        &name,
                        Binding {
                            lazy: None,
                            slot,
                            ty,
                            writable: true,
                            span: name.span,
                        },
                    )?;
                }
                let value = match &arm.body {
                    ast::LambdaBody::Expression(expression) => {
                        self.expression(expression, result_type.clone(), evaluate)?
                    }
                    ast::LambdaBody::Block(block) => {
                        self.block(block, true);
                        self.literal(Scalar::Unit, arm.span)
                    }
                };
                let completes = self.function.reachable_from(plan.block)[self.current];
                Ok((value, completes))
            })();
            self.scopes.pop();
            let (value, completes) = checked?;
            if completes {
                if let Some(expected) = &result_type {
                    self.matches(expected.clone(), value.ty.clone(), arm.span)?;
                } else {
                    result_type = Some(value.ty.clone());
                }
                completed.push((self.current, value));
            }
        }
        let result_type = result_type.unwrap_or(Type::Unit);
        let result = self.local(result_type.clone());
        for (block, value) in completed {
            self.current = block;
            self.emit(
                Inst::Copy {
                    destination: result,
                    source: value.slot,
                },
                span,
            );
            self.terminate(Terminator::Goto(done), span);
        }
        self.current = done;
        Ok(Value {
            slot: result,
            ty: result_type,
            constant: None,
        })
    }
}
