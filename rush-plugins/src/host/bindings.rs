use std::sync::Mutex;

use anyhow::{bail, Context};
use extism::{CurrentPlugin, Function, UserData, Val, ValType};
use lazy_static::lazy_static;

use crate::plugin::{HostBindings, NoOpHostBindings};

lazy_static! {
    // TODO(@doinkythederp): if we ever make plugins multithreaded, make this a thread_local
    /// Global bindings that plugins can call to interact with the shell
    pub static ref HOST_BINDINGS: Mutex<Box<dyn HostBindings>> =
        Mutex::new(Box::new(NoOpHostBindings));

    pub static ref OUTPUT_TEXT_FN: Function = Function::new(
        "output_text",
        [ValType::I64],
        [],
        None,
        output_text,
    );
    pub static ref ENV_GET_FN: Function = Function::new(
        "env_get",
        [ValType::I64],
        [ValType::I64],
        None,
        env_get,
    );
    pub static ref ENV_SET_FN: Function = Function::new(
        "env_set",
        [ValType::I64, ValType::I64],
        [],
        None,
        env_set,
    );
    pub static ref ENV_DELETE_FN: Function = Function::new(
        "env_delete",
        [ValType::I64],
        [],
        None,
        env_delete,
    );
    pub static ref ENV_VARS_FN: Function = Function::new(
        "env_vars",
        [],
        [ValType::I64],
        None,
        env_vars,
    );
    pub static ref FS_IS_EXECUTABLE_FN: Function = Function::new(
        "fs_is_executable",
        [ValType::I64],
        [ValType::I32],
        None,
        fs_is_executable,
    );
}

macro_rules! load_string {
    ($plugin:expr, $offset:expr) => {{
        let mem = $plugin
            .memory
            .at_offset($offset as usize)
            .context("Invalid memory offset")?;
        $plugin
            .memory
            .get_str(mem)
            .context("Invalid string in memory")?
            .to_owned()
    }};
}

#[rustfmt::skip]
macro_rules! get_arg {
    ($args:expr, $index:expr, $ty:ident) => {{
        let Some(arg): Option<$ty> = $args
            .get($index)
            .and_then(|p| p.$ty()) else {
                bail!("Expected a `{}` at argument {}", stringify!($ty), $index);
            };
        arg
    }};
}

pub fn output_text(
    plugin: &mut CurrentPlugin,
    args: &[Val],
    _ret: &mut [Val],
    _user_data: UserData,
) -> Result<(), anyhow::Error> {
    let arg = get_arg!(args, 0, i64);
    let input = load_string!(plugin, arg);

    let mut bindings = HOST_BINDINGS.lock().unwrap();
    bindings.output_text(plugin, input);

    Ok(())
}

pub fn env_get(
    plugin: &mut CurrentPlugin,
    args: &[Val],
    ret: &mut [Val],
    _user_data: UserData,
) -> Result<(), anyhow::Error> {
    let arg0 = get_arg!(args, 0, i64);
    let var_name = load_string!(plugin, arg0);

    let mut bindings = HOST_BINDINGS.lock().unwrap();
    let ret_val = bindings.env_get(plugin, var_name);
    let ret_val = serde_json::to_string(&ret_val)?;
    let ret_memory = plugin.memory.alloc_bytes(ret_val.as_bytes())?;
    ret[0] = Val::I64(ret_memory.offset as i64);

    Ok(())
}

pub fn env_set(
    plugin: &mut CurrentPlugin,
    args: &[Val],
    _ret: &mut [Val],
    _user_data: UserData,
) -> Result<(), anyhow::Error> {
    let arg0 = get_arg!(args, 0, i64);
    let var_name = load_string!(plugin, arg0);
    let arg1 = get_arg!(args, 1, i64);
    let var_value = load_string!(plugin, arg1);

    let mut bindings = HOST_BINDINGS.lock().unwrap();
    bindings.env_set(plugin, var_name, var_value);

    Ok(())
}

pub fn env_delete(
    plugin: &mut CurrentPlugin,
    args: &[Val],
    _ret: &mut [Val],
    _user_data: UserData,
) -> Result<(), anyhow::Error> {
    let arg0 = get_arg!(args, 0, i64);
    let var_name = load_string!(plugin, arg0);

    let mut bindings = HOST_BINDINGS.lock().unwrap();
    bindings.env_delete(plugin, var_name);

    Ok(())
}

pub fn env_vars(
    plugin: &mut CurrentPlugin,
    _args: &[Val],
    ret: &mut [Val],
    _user_data: UserData,
) -> Result<(), anyhow::Error> {
    let mut bindings = HOST_BINDINGS.lock().unwrap();
    let ret_val = bindings.env_vars(plugin);
    let ret_val = serde_json::to_string(&ret_val)?;
    let ret_memory = plugin.memory.alloc_bytes(ret_val.as_bytes())?;
    ret[0] = Val::I64(ret_memory.offset as i64);

    Ok(())
}

pub fn fs_is_executable(
    plugin: &mut CurrentPlugin,
    args: &[Val],
    ret: &mut [Val],
    _user_data: UserData,
) -> Result<(), anyhow::Error> {
    let arg0 = get_arg!(args, 0, i64);
    let path = load_string!(plugin, arg0);

    let mut bindings = HOST_BINDINGS.lock().unwrap();
    let ret_val = bindings.fs_is_executable(plugin, path);
    ret[0] = Val::I32(ret_val.into());
    Ok(())
}
