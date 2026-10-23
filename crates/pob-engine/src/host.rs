use std::io::{BufRead, Write};

use mlua::serde::DeserializeOptions;
use mlua::{Function, Lua, LuaSerdeExt, Table};
use serde_json::Value;

use crate::protocol::{Request, Response};
use crate::{EngineError, PobDir};

const PRELUDE: &str = include_str!("../lua/prelude.lua");
const HEADLESS: &str = include_str!("../lua/headless.lua");
const BRIDGE: &str = include_str!("../lua/bridge.lua");

/// PoB loaded into a LuaJIT state, answering bridge methods. One per process: PoB keeps its state
/// in globals and loads its files relative to the working directory, which this changes.
pub struct Host {
    lua: Lua,
    bridge: Table,
}

impl Host {
    pub fn new(pob: &PobDir) -> Result<Self, EngineError> {
        std::env::set_current_dir(&pob.src).map_err(EngineError::Io)?;
        // PoB needs io, os, package and debug, which the safe constructor leaves out.
        let lua = unsafe { Lua::unsafe_new() };
        let lua_dir = pob.lua.to_string_lossy().replace('\\', "/");
        lua.load(PRELUDE)
            .set_name("@prelude.lua")
            .call::<()>(lua_dir)
            .map_err(lua_err)?;
        register_zlib(&lua).map_err(lua_err)?;
        lua.load(HEADLESS)
            .set_name("@headless.lua")
            .exec()
            .map_err(lua_err)?;
        let bridge: Table = lua
            .load(BRIDGE)
            .set_name("@bridge.lua")
            .eval()
            .map_err(lua_err)?;
        Ok(Self { lua, bridge })
    }

    pub fn call(&self, method: &str, params: &Value) -> Result<Value, EngineError> {
        let func: Function = self
            .bridge
            .get(method)
            .map_err(|_| EngineError::UnknownMethod(method.to_string()))?;
        let params = self.lua.to_value(params).map_err(lua_err)?;
        let result: mlua::Value = func.call(params).map_err(lua_err)?;
        // A Lua table can't tell an empty list from an empty object. Lists are the common case here.
        let opts = DeserializeOptions::new().encode_empty_tables_as_array(true);
        self.lua.from_value_with(result, opts).map_err(lua_err)
    }

    /// Answers one JSON request per line until the input closes. The first line out says the
    /// engine is ready.
    pub fn serve(
        &self,
        input: impl BufRead,
        mut output: impl Write,
        ready: Value,
    ) -> std::io::Result<()> {
        write_line(&mut output, &Response::ok(0, ready))?;
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let response = match serde_json::from_str::<Request>(&line) {
                Ok(req) => match self.call(&req.method, &req.params) {
                    Ok(v) => Response::ok(req.id, v),
                    Err(e) => Response::err(req.id, e.to_string()),
                },
                Err(e) => Response::err(0, format!("bad request: {e}")),
            };
            write_line(&mut output, &response)?;
        }
        Ok(())
    }
}

/// Build codes are base64 of a zlib stream, and so are the timeless and abyss jewel tables. A
/// stream that doesn't decode gives nil, the way PoB's own Inflate does.
fn register_zlib(lua: &Lua) -> mlua::Result<()> {
    use std::io::Read;

    let inflate = lua.create_function(|lua, data: mlua::LuaString| {
        let mut out = Vec::new();
        match flate2::read::ZlibDecoder::new(&data.as_bytes()[..]).read_to_end(&mut out) {
            Ok(_) => Ok(Some(lua.create_string(out)?)),
            Err(_) => Ok(None),
        }
    })?;
    let deflate = lua.create_function(|lua, data: mlua::LuaString| {
        let mut out = Vec::new();
        flate2::read::ZlibEncoder::new(&data.as_bytes()[..], flate2::Compression::default())
            .read_to_end(&mut out)
            .map_err(mlua::Error::external)?;
        lua.create_string(out)
    })?;
    lua.globals().set("Inflate", inflate)?;
    lua.globals().set("Deflate", deflate)
}

fn write_line(output: &mut impl Write, response: &Response) -> std::io::Result<()> {
    serde_json::to_writer(&mut *output, response)?;
    output.write_all(b"\n")?;
    output.flush()
}

fn lua_err(e: mlua::Error) -> EngineError {
    EngineError::Pob(e.to_string())
}
