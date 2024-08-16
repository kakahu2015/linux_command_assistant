use std::collections::HashMap;
use std::error::Error;

pub trait Plugin {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute(&self, args: &[String]) -> Result<String, Box<dyn Error>>;
}

pub struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.insert(plugin.name().to_string(), plugin);
    }

    pub fn execute_plugin(&self, name: &str, args: &[String]) -> Result<String, Box<dyn Error>> {
        if let Some(plugin) = self.plugins.get(name) {
            plugin.execute(args)
        } else {
            Err("Plugin not found".into())
        }
    }
}

pub struct PluginCall {
    pub name: String,
    pub args: Vec<String>,
}
