use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    I32,
    F32,
    Bool,
    Str,
    Void,
}

impl Ty {
    pub fn name(self) -> &'static str {
        match self {
            Ty::I32 => "i32",
            Ty::F32 => "f32",
            Ty::Bool => "bool",
            Ty::Str => "string",
            Ty::Void => "void",
        }
    }

    pub fn from_name(name: &str) -> Option<Ty> {
        match name {
            "i32" => Some(Ty::I32),
            "f32" => Some(Ty::F32),
            "bool" => Some(Ty::Bool),
            "string" => Some(Ty::Str),
            "void" => Some(Ty::Void),
            _ => None,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

pub struct HostFn {
    pub path: &'static str,
    pub params: &'static [Ty],
    pub ret: Ty,
}

pub const HOST_FUNCTIONS: &[HostFn] = &[
    HostFn {
        path: "log",
        params: &[Ty::Str],
        ret: Ty::Void,
    },
    HostFn {
        path: "input.down",
        params: &[Ty::I32],
        ret: Ty::Bool,
    },
    HostFn {
        path: "transform.move",
        params: &[Ty::F32, Ty::F32],
        ret: Ty::Void,
    },
];

pub struct HostConst {
    pub path: &'static str,
    pub value: i32,
}

pub const HOST_CONSTS: &[HostConst] = &[
    HostConst {
        path: "Key.Up",
        value: 0,
    },
    HostConst {
        path: "Key.Down",
        value: 1,
    },
    HostConst {
        path: "Key.Left",
        value: 2,
    },
    HostConst {
        path: "Key.Right",
        value: 3,
    },
    HostConst {
        path: "Key.Space",
        value: 4,
    },
    HostConst {
        path: "Key.Escape",
        value: 5,
    },
];

pub const HOST_ROOTS: &[&str] = &["log", "input", "transform", "Key"];

pub fn host_fn(path: &str) -> Option<&'static HostFn> {
    HOST_FUNCTIONS.iter().find(|f| f.path == path)
}

pub fn host_const(path: &str) -> Option<&'static HostConst> {
    HOST_CONSTS.iter().find(|c| c.path == path)
}
