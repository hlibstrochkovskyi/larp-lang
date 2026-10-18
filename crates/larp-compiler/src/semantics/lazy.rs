//! Pure snapshot-capturing thunks with read-only source bindings.
use super::*;

impl Builder<'_> {
    pub(super) fn lazy(
        &mut self,
        expression_type: &ast::TypeExpr,
        name: &Name,
        expression: &Expr,
        span: Span,
    ) -> Result<()> {
        let result = self.resolve_type(expression_type)?;
        if !result.is_snapshot() {
            return Err(error(
                "LARP-T014",
                expression_type.span,
                "A lazy result must be a SnapshotValue",
            ));
        }
        let body = ast::LambdaBody::Expression(Box::new(expression.clone()));
        let previous = self.snapshot_capture;
        self.snapshot_capture = true;
        let thunk = self.lambda(
            &[],
            &body,
            Some(Type::function(Vec::new(), result.clone(), Effects::ABORT)),
            span,
        );
        self.snapshot_capture = previous;
        let thunk = thunk?;
        let Type::Function(signature) = thunk.ty else {
            unreachable!("typed initializer thunk")
        };
        let cell_type = Type::lazy(result.clone(), signature.effects);
        if !cell_type.within_limits() {
            return Err(error(
                "LARP-P003",
                span,
                "Lazy cell type complexity limit exceeded",
            ));
        }
        let instruction = self.function.blocks[self.current]
            .instructions
            .last_mut()
            .unwrap();
        let Inst::Closure {
            function, captures, ..
        } = &instruction.kind
        else {
            unreachable!("initializer closure emission")
        };
        instruction.kind = Inst::Lazy {
            destination: thunk.slot,
            function: *function,
            captures: captures.clone(),
        };
        self.function.locals[thunk.slot] = cell_type;
        self.generated.last_mut().unwrap().name =
            format!("lazy:{}@{}", name.text, span.range().start);
        self.declare(
            name,
            Binding {
                slot: thunk.slot,
                ty: result,
                lazy: Some(signature.effects),
                writable: false,
                span: name.span,
            },
        )
    }
}
