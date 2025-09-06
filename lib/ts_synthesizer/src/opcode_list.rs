use itertools::Itertools;
use ruse_object_graph::{Number, StringValue, ValueType};
use ruse_synthesizer::{context::VariableName, opcode::OpcodesList};
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

pub const ALL_UNARY_NUM_OPCODES: [ast::UnaryOp; 1] = [
    ast::UnaryOp::Minus,
    // ast::UnaryOp::Tilde,
];

pub const ALL_UPDATE_NUM_OPCODES: [ast::UpdateOp; 2] =
    [ast::UpdateOp::MinusMinus, ast::UpdateOp::PlusPlus];

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

pub const ALL_UNARY_BOOL_OPCODES: [ast::UnaryOp; 1] = [ast::UnaryOp::Bang];

pub const ALL_BIN_STR_OPCODES: [ast::BinaryOp; 1] = [ast::BinaryOp::Add];

pub fn construct_opcode_list<'a, V, N, S>(
    var_names: V,
    num_literals: N,
    string_literals: S,
    add_bool_lit: bool,
) -> OpcodesList
where
    V: std::iter::IntoIterator<Item = &'a VariableName>,
    N: std::iter::IntoIterator<Item = &'a i64>,
    S: std::iter::IntoIterator<Item = &'a StringValue>,
{
    let mut opcodes: OpcodesList = Vec::new();

    // Add variable access
    for var in var_names {
        let op = Arc::new(opcode::IdentOp::new(var.clone()));
        opcodes.push(op);
    }

    // Add number literals
    for n in num_literals {
        opcodes.push(Arc::new(opcode::LitOp::Num(Number::from(*n))));
    }

    // Add string literals
    for s in string_literals {
        let op = Arc::new(opcode::LitOp::Str(s.clone()));
        opcodes.push(op);
    }

    // Add bool literals
    if add_bool_lit {
        opcodes.push(Arc::new(opcode::LitOp::Bool(false)));
        opcodes.push(Arc::new(opcode::LitOp::Bool(true)));
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
        let op = Arc::new(opcode::BinOp::new(
            *op,
            ValueType::Number,
            ValueType::Number,
        ));
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
    opcodes.push(Arc::new(opcode::AssignOp::new(ast::AssignOp::Assign, ValueType::Number)));
}

pub fn add_bool_opcodes(
    opcodes: &mut OpcodesList,
    bin_bool_opcodes: &[ast::BinaryOp],
    unary_bool_opcodes: &[ast::UnaryOp],
) {
    for op in bin_bool_opcodes {
        let op = Arc::new(opcode::BinOp::new(*op, ValueType::Bool, ValueType::Bool));
        opcodes.push(op);
    }
    for op in unary_bool_opcodes {
        let op = Arc::new(opcode::UnaryOp::new(*op, ValueType::Bool));
        opcodes.push(op);
    }
    opcodes.push(Arc::new(opcode::AssignOp::new(ast::AssignOp::Assign, ValueType::Bool)));
}

pub fn add_str_opcodes(opcodes: &mut OpcodesList, str_bool_opcodes: &[ast::BinaryOp]) {
    for op in str_bool_opcodes {
        let op = Arc::new(opcode::BinOp::new(
            *op,
            ValueType::String,
            ValueType::String,
        ));
        opcodes.push(op);
    }
    opcodes.push(Arc::new(opcode::StringLengthOp::new()));
    opcodes.push(Arc::new(opcode::StringSplitOp::new()));
    opcodes.push(Arc::new(opcode::StringSliceOp::new(false)));
    opcodes.push(Arc::new(opcode::StringSliceOp::new(true)));
    opcodes.push(Arc::new(opcode::StringIndexOfOp::new()));
    opcodes.push(Arc::new(opcode::StringLastIndexOfOp::new()));
    opcodes.push(Arc::new(opcode::StringReplaceAllOp::new()));
    opcodes.push(Arc::new(opcode::StringAtOp::new()));
    opcodes.push(Arc::new(opcode::StringSubstringOp::new(false)));
    opcodes.push(Arc::new(opcode::StringSubstringOp::new(true)));
    opcodes.push(Arc::new(opcode::AssignOp::new(ast::AssignOp::Assign, ValueType::String)));
}

pub fn add_array_opcodes(opcodes: &mut OpcodesList, array_types: &[ValueType]) {
    for t in array_types {
        opcodes.push(Arc::new(opcode::ArrayLengthOp::new(t)));
        opcodes.push(Arc::new(opcode::ArrayIndexOp::new(t)));
        opcodes.push(Arc::new(opcode::ArraySliceOp::new(t, false)));
        opcodes.push(Arc::new(opcode::ArraySliceOp::new(t, true)));
        opcodes.push(Arc::new(opcode::ArrayConcatOp::new(t, 1)));
        opcodes.push(Arc::new(opcode::ArrayConcatArrayOp::new(t)));
        opcodes.push(Arc::new(opcode::ArraySpliceOp::new(t, false)));
        opcodes.push(Arc::new(opcode::ArraySpliceOp::new(t, true)));
        opcodes.push(Arc::new(opcode::ArrayPushOp::new(t)));
        opcodes.push(Arc::new(opcode::ArrayPopOp::new(t)));
        opcodes.push(Arc::new(opcode::ArrayReverseOp::new(t)));
        opcodes.push(Arc::new(opcode::ArraySortOp::new(t)));
        opcodes.push(Arc::new(opcode::ArrayShiftOp::new(t)));
        if t.is_primitive() {
            opcodes.push(Arc::new(opcode::ArrayJoinOp::new(t, false)));
            opcodes.push(Arc::new(opcode::ArrayJoinOp::new(t, true)));
        }
    }
}

pub fn add_set_opcodes(opcodes: &mut OpcodesList, set_inner_types: &[ValueType]) {
    for t in set_inner_types {
        opcodes.push(Arc::new(opcode::SetAddOp::new(t)));
        opcodes.push(Arc::new(opcode::SetDeleteOp::new(t)));
        opcodes.push(Arc::new(opcode::SetHasOp::new(t)));
        opcodes.push(Arc::new(opcode::SetSizeOp::new(t)));
    }
}

pub fn add_dom_opcodes(opcodes: &mut OpcodesList) {
    let op = Arc::new(opcode::GetElementByIdOp::new());
    opcodes.push(op);
}

pub fn add_seq_opcodes(
    opcodes: &mut OpcodesList,
    size: usize,
    available_value_types: &Vec<ValueType>,
) {
    for arg_types in (0..size)
        .map(|_| available_value_types.clone().into_iter())
        .multi_cartesian_product()
    {
        opcodes.push(Arc::new(opcode::SequenceOp::new(arg_types)));
    }
}
