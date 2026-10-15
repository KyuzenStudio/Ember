pub const CONST_I32: u8 = 0x01;
pub const CONST_F32: u8 = 0x02;
pub const CONST_TRUE: u8 = 0x03;
pub const CONST_FALSE: u8 = 0x04;
pub const CONST_STR: u8 = 0x05;

pub const LOAD_LOCAL: u8 = 0x10;
pub const STORE_LOCAL: u8 = 0x11;
pub const LOAD_GLOBAL: u8 = 0x12;
pub const STORE_GLOBAL: u8 = 0x13;

pub const ADD_I: u8 = 0x20;
pub const SUB_I: u8 = 0x21;
pub const MUL_I: u8 = 0x22;
pub const DIV_I: u8 = 0x23;
pub const MOD_I: u8 = 0x24;
pub const NEG_I: u8 = 0x25;

pub const ADD_F: u8 = 0x28;
pub const SUB_F: u8 = 0x29;
pub const MUL_F: u8 = 0x2A;
pub const DIV_F: u8 = 0x2B;
pub const MOD_F: u8 = 0x2C;
pub const NEG_F: u8 = 0x2D;

pub const EQ_I: u8 = 0x30;
pub const NE_I: u8 = 0x31;
pub const LT_I: u8 = 0x32;
pub const LE_I: u8 = 0x33;
pub const GT_I: u8 = 0x34;
pub const GE_I: u8 = 0x35;

pub const EQ_F: u8 = 0x38;
pub const NE_F: u8 = 0x39;
pub const LT_F: u8 = 0x3A;
pub const LE_F: u8 = 0x3B;
pub const GT_F: u8 = 0x3C;
pub const GE_F: u8 = 0x3D;

pub const EQ_B: u8 = 0x40;
pub const NE_B: u8 = 0x41;
pub const EQ_S: u8 = 0x44;
pub const NE_S: u8 = 0x45;

pub const AND: u8 = 0x48;
pub const OR: u8 = 0x49;
pub const NOT: u8 = 0x4A;

pub const JUMP: u8 = 0x50;
pub const JUMP_IF_FALSE: u8 = 0x51;

pub const CALL_FUNC: u8 = 0x60;
pub const CALL_HOST: u8 = 0x61;

pub const RETURN: u8 = 0x70;

pub enum Operand {
    None,
    I32,
    F32,
    U16,
    U32,
}

pub fn operand(op: u8) -> Option<Operand> {
    match op {
        CONST_TRUE | CONST_FALSE => Some(Operand::None),
        ADD_I | SUB_I | MUL_I | DIV_I | MOD_I | NEG_I => Some(Operand::None),
        ADD_F | SUB_F | MUL_F | DIV_F | MOD_F | NEG_F => Some(Operand::None),
        EQ_I | NE_I | LT_I | LE_I | GT_I | GE_I => Some(Operand::None),
        EQ_F | NE_F | LT_F | LE_F | GT_F | GE_F => Some(Operand::None),
        EQ_B | NE_B | EQ_S | NE_S => Some(Operand::None),
        AND | OR | NOT => Some(Operand::None),
        RETURN => Some(Operand::None),
        CONST_I32 => Some(Operand::I32),
        CONST_F32 => Some(Operand::F32),
        CONST_STR | LOAD_LOCAL | STORE_LOCAL | LOAD_GLOBAL | STORE_GLOBAL | CALL_FUNC
        | CALL_HOST => Some(Operand::U16),
        JUMP | JUMP_IF_FALSE => Some(Operand::U32),
        _ => None,
    }
}

pub fn len(op: u8) -> Option<usize> {
    match operand(op)? {
        Operand::None => Some(1),
        Operand::I32 | Operand::F32 | Operand::U32 => Some(5),
        Operand::U16 => Some(3),
    }
}

pub fn name(op: u8) -> Option<&'static str> {
    match op {
        CONST_I32 => Some("const.i32"),
        CONST_F32 => Some("const.f32"),
        CONST_TRUE => Some("const.true"),
        CONST_FALSE => Some("const.false"),
        CONST_STR => Some("const.str"),
        LOAD_LOCAL => Some("load.local"),
        STORE_LOCAL => Some("store.local"),
        LOAD_GLOBAL => Some("load.global"),
        STORE_GLOBAL => Some("store.global"),
        ADD_I => Some("add.i"),
        SUB_I => Some("sub.i"),
        MUL_I => Some("mul.i"),
        DIV_I => Some("div.i"),
        MOD_I => Some("mod.i"),
        NEG_I => Some("neg.i"),
        ADD_F => Some("add.f"),
        SUB_F => Some("sub.f"),
        MUL_F => Some("mul.f"),
        DIV_F => Some("div.f"),
        MOD_F => Some("mod.f"),
        NEG_F => Some("neg.f"),
        EQ_I => Some("eq.i"),
        NE_I => Some("ne.i"),
        LT_I => Some("lt.i"),
        LE_I => Some("le.i"),
        GT_I => Some("gt.i"),
        GE_I => Some("ge.i"),
        EQ_F => Some("eq.f"),
        NE_F => Some("ne.f"),
        LT_F => Some("lt.f"),
        LE_F => Some("le.f"),
        GT_F => Some("gt.f"),
        GE_F => Some("ge.f"),
        EQ_B => Some("eq.b"),
        NE_B => Some("ne.b"),
        EQ_S => Some("eq.s"),
        NE_S => Some("ne.s"),
        AND => Some("and"),
        OR => Some("or"),
        NOT => Some("not"),
        JUMP => Some("jump"),
        JUMP_IF_FALSE => Some("jump.if_false"),
        CALL_FUNC => Some("call.func"),
        CALL_HOST => Some("call.host"),
        RETURN => Some("return"),
        _ => None,
    }
}
