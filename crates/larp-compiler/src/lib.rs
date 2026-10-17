//! Name resolution, scalar type/effect checking, typed IR, and verified bytecode.

mod semantics;

use larp_bytecode::VerifiedModule;
use larp_diagnostics::Diagnostic;
use larp_source::SourceFile;

pub struct Compilation {
    pub ir: larp_ir::Program,
    pub bytecode: VerifiedModule,
}

pub fn compile(source: &SourceFile) -> Result<Compilation, Vec<Diagnostic>> {
    let ast = larp_syntax::parse(source)?;
    let ir = semantics::check(&ast)?;
    let module = larp_ir::lower(&ir);
    let bytecode = larp_bytecode::verify(module).map_err(|error| {
        let function = &ir.functions[error.function.min(ir.functions.len() - 1)];
        let resource_limit = error.kind == larp_bytecode::VerifyErrorKind::ResourceLimit;
        vec![Diagnostic::error(
            if resource_limit {
                "LARP-P003"
            } else {
                "LARP-B001"
            },
            function.span,
            format!(
                "Bytecode validation for {}: {}",
                function.name, error.message
            ),
        )]
    })?;
    Ok(Compilation { ir, bytecode })
}
