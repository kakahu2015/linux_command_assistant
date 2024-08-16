use anyhow::{Context, Result};
use reqwest::Client;
use reqwest::tls::Version;
use reqwest::ClientBuilder;
use rustyline::Editor;
use rustyline::config::Config as RustylineConfig;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::process::Command;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
mod completer;
use completer::LinuxCommandCompleter;
use rustyline::error::ReadlineError;
use std::time::Instant;
use std::env;
use std::path::PathBuf;

const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";
// 添加新的常量
const BLUE: &str = "\x1b[34m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";

#[derive(Debug, Deserialize)]
struct Config {
    openai: OpenAIConfig,
    system_prompt: String,
    max_recent_interactions: usize,
    max_openai_context: usize,
}

#[derive(Debug, Deserialize)]
struct OpenAIConfig {
    api_key: String,
    api_base: String,
    model: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

struct LinuxCommandAssistant {
    config: Config,
    client: Client,
    context: Vec<Message>,
    recent_interactions: VecDeque<String>,
    command_history: Vec<String>,
    is_command_mode: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

impl LinuxCommandAssistant {
    fn new(config: Config) -> Result<Self> {
         let client = Client::builder()
            .use_rustls_tls()
            .build()
            .context("Failed to build HTTP client")?;
        
        
        let context = vec![Message {
            role: "system".to_string(),
            content: config.system_prompt.clone(),
        }];
        let max_recent_interactions = config.max_recent_interactions;
        Ok(Self {
            config,
            client,
            context,
            recent_interactions: VecDeque::with_capacity(max_recent_interactions),
            command_history: Vec::new(),
            is_command_mode: false,
        })
    }

    /////////////////////////////
async fn get_ai_response(&mut self, prompt: &str) -> Result<String> {
    // println!("Entering get_ai_response function");
    
    let mut messages = self.context.clone();
    if messages.is_empty() {
        // println!("Context is empty, adding system prompt");
        messages.push(Message {
            role: "system".to_string(),
            content: self.config.system_prompt.clone(),
        });
    }
    
    if !self.recent_interactions.is_empty() {
        // println!("Adding recent interactions to context");
        let recent_history = self.recent_interactions.iter().cloned().collect::<Vec<_>>().join("\n");
        messages.push(Message {
            role: "user".to_string(),
            content: format!("Recent interactions:\n{}\nPlease consider this context for the following question about Linux commands.", recent_history),
        });
    }
    
    messages.push(Message {
        role: "user".to_string(),
        content: prompt.to_string(),
    });

    // println!("Preparing request payload");
    let request = serde_json::json!({
        "model": self.config.openai.model,
        "messages": messages,
    });

    // println!("Setting up headers");
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", self.config.openai.api_key))
        .context("Failed to create Authorization header")?);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    // println!("Sending request to OpenAI API: {}", self.config.openai.api_base);
    let start = Instant::now();
    let response = self.client.post(&self.config.openai.api_base)
        .headers(headers)
        .json(&request)
        .send()
        .await;

    match response {
        Ok(resp) => {
            // println!("Request completed in {:?}", start.elapsed());
            let status = resp.status();
            // println!("Response status: {}", status);
            // println!("Response headers: {:?}", resp.headers());

            let body = resp.text().await.context("Failed to read response body")?;
            // println!("Response body: {}", body);

            if status.is_success() {
                let response: serde_json::Value = serde_json::from_str(&body)
                    .context("Failed to parse API response")?;
                
                if let Some(content) = response["choices"][0]["message"]["content"].as_str() {
                    Ok(content.to_string())
                } else {
                    Err(anyhow::anyhow!("No response content from AI"))
                }
            } else {
                Err(anyhow::anyhow!("API request failed with status {}: {}", status, body))
            }
        },
        Err(e) => {
            // println!("Request failed after {:?}", start.elapsed());
            // println!("Error details: {:?}", e);
            if let Some(url) = e.url() {
                // println!("Failed URL: {}", url);
            }
            Err(anyhow::anyhow!("Failed to send request: {}", e))
        }
    }
}
    /////////////////////////////////////////////////////////////
fn colorize_ls_output(&self, output: &str) -> String {
        output.lines().map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 9 {
                let permissions = parts[0];
                let filename = parts[8..].join(" ");
                let colored_filename = if permissions.starts_with('d') {
                    format!("{}{}{}", BLUE, filename, RESET)
                } else if permissions.contains('x') {
                    format!("{}{}{}", GREEN, filename, RESET)
                } else {
                    format!("{}{}{}", RED, filename, RESET)
                };
                let mut colored_line = parts[..8].join(" ");
                colored_line.push_str(" ");
                colored_line.push_str(&colored_filename);
                colored_line
            } else {
                line.to_string()
            }
        }).collect::<Vec<String>>().join("\n")
    }

    fn execute_command(&self, command: &str) -> Result<String> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .context("Failed to execute command")?;
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        
        if output.status.success() {
            if stdout.is_empty() {
                Ok(stderr)
            } else {
                //if command.trim() == "ls -l" {
                if command.trim().starts_with("ls") && command.contains("-l") {
                    Ok(self.colorize_ls_output(&stdout))
                } else {
                    Ok(stdout)
                }
            }
        } else {
            if stderr.is_empty() {
                Ok(stdout)
            } else {
                Ok(stderr)
            }
        }
    }


fn update_context(&mut self, user_input: &str, response: &str) {
    // 清除之前的命令历史
    self.context.retain(|msg| msg.role != "user" || !msg.content.starts_with("Recent interactions:"));

    // 添加新的交互
    let recent_history = self.recent_interactions.iter().cloned().collect::<Vec<_>>().join("\n");
    self.context.push(Message {
        role: "user".to_string(),
        content: format!("Recent interactions:\n{}", recent_history),
    });
    self.context.push(Message {
        role: "user".to_string(),
        content: user_input.to_string(),
    });
    self.context.push(Message {
        role: "assistant".to_string(),
        content: response.to_string(),
    });

    // 保持上下文大小在限制内
    while self.context.len() > self.config.max_openai_context {
        self.context.remove(0);
    }
}

    fn add_to_recent_interactions(&mut self, interaction: String) {
        self.recent_interactions.push_back(interaction);
        if self.recent_interactions.len() > self.config.max_recent_interactions {
            self.recent_interactions.pop_front();
        }
    }
///////////////////////////run start/////////////////////////
async fn run(&mut self) -> Result<()> {
    let config = RustylineConfig::builder()
        .history_ignore_space(true)
        .completion_type(rustyline::CompletionType::List)
        .build();
    let mut rl = Editor::with_config(config)?;
    rl.set_helper(Some(LinuxCommandCompleter));

    loop {
        let readline = rl.readline(&format!("{}kaka-ai> {}", YELLOW, RESET));
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.eq_ignore_ascii_case("exit") {
                    break;
                }

                if line.eq_ignore_ascii_case("reset") {
                   // 清空所有上下文
                   self.context.clear();
                   // 清空最近交互
                   self.recent_interactions.clear();
                    println!("Context and recent interactions have been reset.");
                    continue;
                }

                if !line.is_empty() && !line.starts_with('#') {
                    self.add_to_history(line.to_string());
                    rl.add_history_entry(line);
                }

                if line.starts_with('!') {
                    let command = &line[1..];
                    match self.execute_command(command) {
                        Ok(output) => {
                            println!("{}", output);
                            self.add_to_recent_interactions(format!("Command: {}\nOutput: {}", command, output));
                        }
                        Err(e) => println!("Error executing command: {}", e),
                    }
                } else {
                    match self.get_ai_response(line).await {
                        Ok(response) => {
                            println!("kaka-AI: {}", response);
                            self.update_context(line, &response);
                            self.add_to_recent_interactions(format!("User: {}\nAI: {}", line, response));
                        }
                        Err(e) => println!("Error getting AI response: {}", e),
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
    //////////////////////////////////run end////
        // 添加新方法
    fn add_to_history(&mut self, command: String) {
        self.command_history.push(command);
    }
    ////////////////////////////////////////////////
}

fn load_config() -> Result<Config> {
    // 新增: 获取可执行文件路径
    let exe_path = env::current_exe().context("Failed to get executable path")?;
    // 新增: 获取可执行文件所在目录
    let exe_dir = exe_path.parent().context("Failed to get executable directory")?;
    // 新增: 构建配置文件的完整路径
    let config_path = exe_dir.join("config.yml");

    // 修改: 使用新的配置文件路径
    let config_str = std::fs::read_to_string(&config_path)
        .context("Failed to read config.yml")?;
    // 不变: 解析配置文件的逻辑保持不变
    let config: Config = serde_yaml::from_str(&config_str)
        .context("Failed to parse config.yml")?;
    Ok(config)
}


#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config()?;
    let mut assistant = LinuxCommandAssistant::new(config)?;

    let mut rl = Editor::<LinuxCommandCompleter>::new()?;
    rl.set_helper(Some(LinuxCommandCompleter));

    loop {
        let prompt = if assistant.is_command_mode { "$ " } else { "kaka-ai> " };
        let readline = rl.readline(&format!("{}{}{}", YELLOW, prompt, RESET));

        match readline {
            Ok(line) => {
                let line = line.trim();

                if line.eq_ignore_ascii_case("exit") {
                    break;
                }

                if line == "!" {
                    assistant.is_command_mode = true;
                    println!("Entered Linux command mode. Type 'quit' to exit.");
                    continue;
                }

                if assistant.is_command_mode {
                    if line == "quit" {
                        assistant.is_command_mode = false;
                        println!("Exited Linux command mode.");
                        continue;
                    }

                    match assistant.execute_command(line) {
                        Ok(output) => println!("{}", output),
                        Err(e) => println!("Error executing command: {}", e),
                    }
                } else {
                    match assistant.get_ai_response(line).await {
                        Ok(response) => {
                            println!("kaka-AI: {}", response);
                            assistant.update_context(line, &response);
                            assistant.add_to_recent_interactions(format!("User: {}\nAI: {}", line, response));
                        }
                        Err(e) => println!("Error getting AI response: {}", e),
                    }
                }

                if !line.is_empty() && !line.starts_with('#') {
                    assistant.add_to_history(line.to_string());
                    rl.add_history_entry(line);
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
