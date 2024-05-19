use ruse_object_graph::{CachedString, Number};
use ruse_synthesizer::{synthesizer::OpcodesList, value::ValueType};
use ruse_ts_interpreter::opcode;
use std::sync::Arc;

use swc_ecma_ast as ast;

pub const ALL_BIN_NUM_OPCODES: [ast::BinaryOp; 4] = [
    // ast::BinaryOp::NotEq,
    // ast::BinaryOp::EqEqEq,
    // ast::BinaryOp::NotEqEq,
    // ast::BinaryOp::Lt,
    // ast::BinaryOp::LtEq,
    // ast::BinaryOp::Gt,
    // ast::BinaryOp::GtEq,
    // ast::BinaryOp::LShift,
    // ast::BinaryOp::RShift,
    // ast::BinaryOp::ZeroFillRShift,
    ast::BinaryOp::Add,
    ast::BinaryOp::Sub,
    ast::BinaryOp::Mul,
    ast::BinaryOp::Div,
    // ast::BinaryOp::Mod,
    // ast::BinaryOp::BitOr,
    // ast::BinaryOp::BitXor,
    // ast::BinaryOp::BitAnd,
    // ast::BinaryOp::Exp,
];

pub const ALL_UNARY_NUM_OPCODES: [ast::UnaryOp; 2] = [
    ast::UnaryOp::Minus,
    ast::UnaryOp::Tilde,
];

pub const ALL_UPDATE_NUM_OPCODES: [ast::UpdateOp; 2] = [
    ast::UpdateOp::MinusMinus,
    ast::UpdateOp::PlusPlus,
];

pub const ALL_BIN_BOOL_OPCODES: [ast::BinaryOp; 9] = [
    ast::BinaryOp::EqEq,
    ast::BinaryOp::NotEq,
    ast::BinaryOp::EqEqEq,
    ast::BinaryOp::NotEqEq,
    ast::BinaryOp::Lt,
    ast::BinaryOp::LtEq,
    ast::BinaryOp::Gt,
    ast::BinaryOp::GtEq,
    ast::BinaryOp::Add,
];

pub const ALL_UNARY_BOOL_OPCODES: [ast::UnaryOp; 1] = [
    ast::UnaryOp::Bang,
];

pub const ALL_BIN_STR_OPCODES: [ast::BinaryOp; 1] = [
    ast::BinaryOp::Add,
];

pub fn construct_opcode_list(
    var_names: &[CachedString],
    num_literals: &[f64],
    string_literals: &[CachedString],
    add_bool_lit: bool,
) -> OpcodesList {
    let mut opcodes: OpcodesList = Vec::new();

    // Add variable access
    for var in var_names {
        let op = Arc::new(opcode::IdentOp { name: var.clone() });
        opcodes.push(op);
    }

    // Add number literals
    for n in num_literals {
        opcodes.push(Arc::new(opcode::LitOp::Num(Number(*n))));
    }

    // Add bool literals
    if add_bool_lit {
        opcodes.push(Arc::new(opcode::LitOp::Bool(false)));
        opcodes.push(Arc::new(opcode::LitOp::Bool(true)));
    }

    // Add string literals
    for str in string_literals {
        let op = Arc::new(opcode::LitOp::Str(str.clone()));
        opcodes.push(op);
    }

    opcodes
}

pub fn add_num_opcodes(
    opcodes: &mut OpcodesList,
    bin_num_opcodes: &[ast::BinaryOp],
    unary_num_opcodes: &[ast::UnaryOp],
    update_num_opcodes: &[ast::UpdateOp],
) {
    for op in bin_num_opcodes {
        let op = Arc::new(opcode::BinOp {
            arg_types: [ValueType::Number, ValueType::Number],
            op: *op,
        });
        opcodes.push(op);
    }
    for op in unary_num_opcodes {
        let op = Arc::new(opcode::UnaryOp::new(*op, ValueType::Number));
        opcodes.push(op);
    }
    for op in update_num_opcodes {
        let op = Arc::new(opcode::UpdateOp::new(*op, true));
        opcodes.push(op);
    }
}

pub fn add_bool_opcodes(
    opcodes:  &mut OpcodesList,
    bin_bool_opcodes: &[ast::BinaryOp],
    unary_bool_opcodes: &[ast::UnaryOp],
) {
    for op in bin_bool_opcodes {
        let op = Arc::new(opcode::BinOp {
            arg_types: [ValueType::Bool, ValueType::Bool],
            op: *op,
        });
        opcodes.push(op);
    }
    for op in unary_bool_opcodes {
        let op = Arc::new(opcode::UnaryOp::new(*op, ValueType::Bool));
        opcodes.push(op);
    }
}

pub fn add_str_opcodes(
    opcodes:  &mut OpcodesList,
    str_bool_opcodes: &[ast::BinaryOp],
) {
    for op in str_bool_opcodes {
        let op = Arc::new(opcode::BinOp {
            arg_types: [ValueType::String, ValueType::String],
            op: *op,
        });
        opcodes.push(op);
    }
}
