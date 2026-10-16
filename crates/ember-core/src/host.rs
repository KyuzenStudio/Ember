use super::Ty;

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
    HostFn {
        path: "input.pressed",
        params: &[Ty::I32],
        ret: Ty::Bool,
    },
    HostFn {
        path: "transform.x",
        params: &[],
        ret: Ty::F32,
    },
    HostFn {
        path: "transform.y",
        params: &[],
        ret: Ty::F32,
    },
    HostFn {
        path: "transform.set",
        params: &[Ty::F32, Ty::F32],
        ret: Ty::Void,
    },
    HostFn {
        path: "entity.x",
        params: &[Ty::Str],
        ret: Ty::F32,
    },
    HostFn {
        path: "entity.y",
        params: &[Ty::Str],
        ret: Ty::F32,
    },
    HostFn {
        path: "entity.destroy",
        params: &[Ty::Str],
        ret: Ty::Void,
    },
    HostFn {
        path: "str.of_i",
        params: &[Ty::I32],
        ret: Ty::Str,
    },
    HostFn {
        path: "str.cat",
        params: &[Ty::Str, Ty::Str],
        ret: Ty::Str,
    },
    HostFn {
        path: "audio.play",
        params: &[Ty::Str],
        ret: Ty::Void,
    },
    HostFn {
        path: "audio.loop",
        params: &[Ty::Str],
        ret: Ty::Void,
    },
    HostFn {
        path: "audio.stop",
        params: &[],
        ret: Ty::Void,
    },
    HostFn {
        path: "anim.play",
        params: &[Ty::Str],
        ret: Ty::Void,
    },
    HostFn {
        path: "game.restart",
        params: &[],
        ret: Ty::Void,
    },
    HostFn {
        path: "game.set_title",
        params: &[Ty::Str],
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

pub const HOST_ROOTS: &[&str] = &[
    "log",
    "input",
    "transform",
    "Key",
    "entity",
    "str",
    "audio",
    "anim",
    "game",
];

pub fn host_fn(path: &str) -> Option<&'static HostFn> {
    HOST_FUNCTIONS.iter().find(|f| f.path == path)
}

pub fn host_const(path: &str) -> Option<&'static HostConst> {
    HOST_CONSTS.iter().find(|c| c.path == path)
}
