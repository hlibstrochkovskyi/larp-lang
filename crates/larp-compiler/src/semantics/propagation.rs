use super::*;

impl Builder<'_> {
    pub(super) fn propagate(
        &mut self,
        operand: &Expr,
        expected: Option<Type>,
        span: Span,
        evaluate: bool,
    ) -> Result<Value> {
        if self.scenario_boundary {
            return Err(error(
                "LARP-S005",
                span,
                "Result propagation cannot cross a scenario boundary",
            ));
        }
        if self.function.constructor.is_some() {
            return Err(error(
                "LARP-T012",
                span,
                "A constructor cannot return a propagated result",
            ));
        }
        if self.inferred_result.is_some() {
            let mut diagnostic = error(
                "LARP-T017",
                span,
                "Result propagation requires a known enclosing result type",
            );
            diagnostic.help =
                Some("Give the lambda an explicit fn(...) -> result<U,E> context".into());
            return Err(diagnostic);
        }
        let outer = self.function.result.clone();
        let Type::Result(outer_payloads) = &outer else {
            return Err(error(
                "LARP-T017",
                span,
                "Result propagation requires an enclosing result<U,E> return boundary",
            ));
        };
        let error_type = outer_payloads.1.clone();
        let success_context = expected.or_else(|| match self.hint(operand) {
            Some(Type::Result(pair)) => Some(pair.0.clone()),
            _ => None,
        });
        let context = success_context.map(|success| Type::result(success, error_type.clone()));
        // The result's own error type must be checked exactly below. Do not
        // apply an ordinary function-effect or nominal-role coercion to E.
        let value = self.expression_value(operand, context, evaluate)?;
        let Type::Result(payloads) = &value.ty else {
            return Err(error(
                "LARP-T017",
                operand.span,
                "The propagation operand must have type result<T,E>",
            ));
        };
        if payloads.1 != error_type {
            let mut diagnostic = error(
                "LARP-T017",
                span,
                "Propagated error type must exactly match the enclosing result error type",
            );
            diagnostic.expected_type = Some(error_type.to_string());
            diagnostic.found_type = Some(payloads.1.to_string());
            return Err(diagnostic);
        }
        let success_type = payloads.0.clone();
        let success = self.new_block(span);
        let failure = self.new_block(span);
        let success_payload = self.local(success_type.clone());
        let error_payload = self.local(error_type);
        self.function.blocks[success].parameters = vec![success_payload];
        self.function.blocks[failure].parameters = vec![error_payload];
        self.terminate(
            Terminator::MatchResult {
                value: value.slot,
                success,
                error: failure,
            },
            span,
        );
        self.current = failure;
        let result = self.intrinsic(Intrinsic::Err(outer), vec![error_payload], span)?;
        self.unwind_silence(0, span);
        self.terminate(Terminator::Return(Some(result.slot)), span);
        self.current = success;
        Ok(Value {
            slot: success_payload,
            ty: success_type,
            constant: None,
        })
    }
}
