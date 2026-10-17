//! Typed control-flow IR with explicit local slots and backend lowering.

use larp_source::Span;
use larp_types::{Binary, Effects, Scalar, Type, Unary};

pub type Local = usize;
pub type BlockId = usize;

#[derive(Debug)]
pub struct Program {
    pub functions: Vec<Function>,
    pub entry: usize,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Type>,
    pub result: Type,
    pub effects: Effects,
    pub locals: Vec<Type>,
    pub blocks: Vec<Block>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Block {
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
    pub span: Span,
}

#[derive(Debug)]
pub struct Instruction {
    pub kind: InstructionKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum InstructionKind {
    Constant {
        destination: Local,
        value: Scalar,
    },
    Copy {
        destination: Local,
        source: Local,
    },
    Unary {
        destination: Local,
        op: Unary,
        operand: Local,
    },
    Binary {
        destination: Local,
        op: Binary,
        left: Local,
        right: Local,
    },
    Call {
        destination: Local,
        function: usize,
        arguments: Vec<Local>,
    },
    Print(Local),
    Assert(Local),
}

#[derive(Debug)]
pub enum Terminator {
    Unreachable,
    Goto(BlockId),
    Branch {
        condition: Local,
        when_true: BlockId,
        when_false: BlockId,
    },
    Return(Option<Local>),
    Crash(Local),
}

impl Function {
    pub fn reachable(&self) -> Vec<bool> {
        let mut visited = vec![false; self.blocks.len()];
        let mut pending = vec![0];
        while let Some(id) = pending.pop() {
            if visited[id] {
                continue;
            }
            visited[id] = true;
            match self.blocks[id].terminator {
                Terminator::Goto(target) => pending.push(target),
                Terminator::Branch {
                    when_true,
                    when_false,
                    ..
                } => {
                    pending.push(when_true);
                    pending.push(when_false);
                }
                _ => {}
            }
        }
        visited
    }
}

pub fn lower(program: &Program) -> larp_bytecode::Module {
    use larp_bytecode::{Function as ByteFunction, Instruction as ByteInstruction, Op};
    let functions = program
        .functions
        .iter()
        .map(|function| {
            let reachable = function.reachable();
            let mut code = Vec::new();
            let mut offsets = vec![0; function.blocks.len()];
            let mut patches = Vec::new();
            for (id, block) in function.blocks.iter().enumerate() {
                if !reachable[id] {
                    continue;
                }
                offsets[id] = code.len();
                for instruction in &block.instructions {
                    let span = instruction.span;
                    let mut emit = |op| code.push(ByteInstruction { op, span });
                    match &instruction.kind {
                        InstructionKind::Constant { destination, value } => {
                            emit(Op::Constant(value.clone()));
                            emit(Op::Store(*destination));
                        }
                        InstructionKind::Copy {
                            destination,
                            source,
                        } => {
                            emit(Op::Load(*source));
                            emit(Op::Store(*destination));
                        }
                        InstructionKind::Unary {
                            destination,
                            op,
                            operand,
                        } => {
                            emit(Op::Load(*operand));
                            emit(Op::Unary(*op, function.locals[*operand]));
                            emit(Op::Store(*destination));
                        }
                        InstructionKind::Binary {
                            destination,
                            op,
                            left,
                            right,
                        } => {
                            emit(Op::Load(*left));
                            emit(Op::Load(*right));
                            emit(Op::Binary(*op, function.locals[*left]));
                            emit(Op::Store(*destination));
                        }
                        InstructionKind::Call {
                            destination,
                            function,
                            arguments,
                        } => {
                            for argument in arguments {
                                emit(Op::Load(*argument));
                            }
                            emit(Op::Call(*function));
                            emit(Op::Store(*destination));
                        }
                        InstructionKind::Print(value) => {
                            emit(Op::Load(*value));
                            emit(Op::Print);
                        }
                        InstructionKind::Assert(value) => {
                            emit(Op::Load(*value));
                            emit(Op::Assert);
                        }
                    }
                }
                let span = block.span;
                match block.terminator {
                    Terminator::Unreachable => {}
                    Terminator::Goto(target) => {
                        patches.push((code.len(), target));
                        code.push(ByteInstruction {
                            op: Op::Jump(0),
                            span,
                        });
                    }
                    Terminator::Branch {
                        condition,
                        when_true,
                        when_false,
                    } => {
                        code.push(ByteInstruction {
                            op: Op::Load(condition),
                            span,
                        });
                        patches.push((code.len(), when_false));
                        code.push(ByteInstruction {
                            op: Op::JumpIfFalse(0),
                            span,
                        });
                        patches.push((code.len(), when_true));
                        code.push(ByteInstruction {
                            op: Op::Jump(0),
                            span,
                        });
                    }
                    Terminator::Return(value) => {
                        code.push(ByteInstruction {
                            op: value.map_or(Op::Constant(Scalar::Unit), Op::Load),
                            span,
                        });
                        code.push(ByteInstruction {
                            op: Op::Return,
                            span,
                        });
                    }
                    Terminator::Crash(value) => {
                        code.push(ByteInstruction {
                            op: Op::Load(value),
                            span,
                        });
                        code.push(ByteInstruction {
                            op: Op::Crash,
                            span,
                        });
                    }
                }
            }
            for (pc, target) in patches {
                match &mut code[pc].op {
                    Op::Jump(offset) | Op::JumpIfFalse(offset) => *offset = offsets[target],
                    _ => unreachable!("only jumps require block patches"),
                }
            }
            ByteFunction {
                name: function.name.clone(),
                parameters: function.parameters.clone(),
                result: function.result,
                effects: function.effects,
                locals: function.locals.clone(),
                code,
                span: function.span,
            }
        })
        .collect();
    larp_bytecode::Module {
        functions,
        entry: program.entry,
    }
}
