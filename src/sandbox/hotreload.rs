use anyhow::Result;
use libloading::Library;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{path::PathBuf, sync::mpsc, time::Duration};

/// Hot-reloadable module trait
pub trait HotReloadable: Send + Sync {
    fn name(&self) -> &str;
    fn reload(&mut self) -> Result<()>;
}

/// Manages hot-reloading of a dynamic library
pub struct HotReloader {
    lib_path: PathBuf,
    library: Option<Library>,
    watcher: Option<RecommendedWatcher>,
    rx: Option<mpsc::Receiver<notify::Result<Event>>>,
}

impl HotReloader {
    pub fn new<P: Into<PathBuf>>(lib_path: P) -> Result<Self> {
        let lib_path = lib_path.into();
        let (tx, rx) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        watcher.watch(lib_path.parent().unwrap_or(&lib_path), RecursiveMode::NonRecursive)?;

        Ok(Self {
            lib_path,
            library: None,
            watcher: Some(watcher),
            rx: Some(rx),
        })
    }

    pub fn load<F, T>(&mut self, init_fn: F) -> Result<T>
    where
        F: FnOnce(&Library) -> Result<T>,
    {
        // Platform-specific library name
        let lib_name = if cfg!(target_os = "windows") {
            self.lib_path.with_extension("dll")
        } else if cfg!(target_os = "macos") {
            self.lib_path.with_extension("dylib")
        } else {
            self.lib_path.with_extension("so")
        };

        let library = unsafe { Library::new(&lib_name)? };
        let result = init_fn(&library)?;
        self.library = Some(library);
        Ok(result)
    }

    pub fn check_reload(&mut self) -> Result<bool> {
        if let Some(rx) = &self.rx {
            while let Ok(Ok(event)) = rx.try_recv() {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    if event.paths.iter().any(|p| p == &self.lib_path) {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    pub fn reload<F, T>(&mut self, init_fn: F) -> Result<T>
    where
        F: FnOnce(&Library) -> Result<T>,
    {
        // Close old library
        self.library = None;

        // Small delay to ensure file is fully written
        std::thread::sleep(Duration::from_millis(100));

        // Load new version
        self.load(init_fn)
    }
}

/// Lua-based hot-reloading for configuration/scripts
pub struct LuaSandbox {
    lua: mlua::Lua,
    script_path: PathBuf,
}

impl LuaSandbox {
    pub fn new<P: Into<PathBuf>>(script_path: P) -> Result<Self> {
        let script_path = script_path.into();
        let lua = mlua::Lua::new();

        // Load initial script
        if script_path.exists() {
            let script = std::fs::read_to_string(&script_path)?;
            lua.load(&script).exec()?;
        }

        Ok(Self { lua, script_path })
    }

    pub fn reload(&mut self) -> Result<()> {
        if self.script_path.exists() {
            let script = std::fs::read_to_string(&self.script_path)?;
            self.lua.load(&script).exec()?;
        }
        Ok(())
    }

    pub fn lua(&self) -> &mlua::Lua {
        &self.lua
    }

    pub fn call_fn<'lua, R>(&'lua self, name: &str, args: impl mlua::IntoLuaMulti<'lua>) -> Result<R>
    where
        R: mlua::FromLuaMulti<'lua>,
    {
        let func: mlua::Function = self.lua.globals().get(name)?;
        Ok(func.call(args)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lua_sandbox() {
        let mut sandbox = LuaSandbox::new("test_script.lua").unwrap();
        sandbox.lua.load(r#"
            function greet(name)
                return "Hello, " .. name .. "!"
            end
        "#).exec().unwrap();

        let result: String = sandbox.call_fn("greet", ("World",)).unwrap();
        assert_eq!(result, "Hello, World!");
    }
}