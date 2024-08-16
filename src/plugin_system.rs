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


// 在 plugin_system.rs 文件末尾添加

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn name(&self) -> &str {
        "weather"
    }

    fn description(&self) -> &str {
        "Get current weather information for a city"
    }

    fn execute(&self, args: &[String]) -> Result<String, Box<dyn Error>> {
        if args.is_empty() {
            return Err("Please provide a city name".into());
        }
        let city = &args[0];
        // 这里应该是实际的天气API调用，为了示例，我们只返回一个模拟的结果
        Ok(format!("The weather in {} is sunny and 25°C", city))
    }
}
