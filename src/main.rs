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

    async fn get_ai_response(&mut self, prompt: &str) -> Result<String> {
        // Existing implementation remains unchanged
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

    fn update_context(&mut self, user_input: &str, response: &str) {
        self.context.push(Message {
            role: "user".to_string(),
            content: user_input.to_string(),
        });
        self.context.push(Message {
            role: "assistant".to_string(),
            content: response.to_string(),
        });
        if self.context.len() > self.config.max_openai_context {
            self.context = self.context.split_off(self.context.len() - self.config.max_openai_context);
        }
    }

    fn add_to_recent_interactions(&mut self, interaction: String) {
        self.recent_interactions.push_back(interaction);
        if self.recent_interactions.len() > self.config.max_recent_interactions {
            self.recent_interactions.pop_front();
        }
    }

    fn add_to_history(&mut self, command: String) {
        self.command_history.push(command);
    }

    async fn run(&mut self) -> Result<()> {
        let config = RustylineConfig::builder()
            .history_ignore_space(true)
            .completion_type(rustyline::CompletionType::List)
            .build();
        let mut rl = Editor::with_config(config)?;
        rl.set_helper(Some(LinuxCommandCompleter));

        loop {
            let prompt = if self.is_command_mode { 
                format!("{}$ {}", BLUE, RESET) 
            } else { 
                format!("{}kaka-ai> {}", YELLOW, RESET) 
            };
            let readline = rl.readline(&prompt);

            match readline {
                Ok(line) => {
                    let line = line.trim();
                    if line.eq_ignore_ascii_case("exit") {
                        break;
                    }

                    if line.eq_ignore_ascii_case("reset") {
                        self.context.clear();
                        self.recent_interactions.clear();
                        println!("Context and recent interactions have been reset.");
                        continue;
                    }

                    if !line.is_empty() && !line.starts_with('#') {
                        self.add_to_history(line.to_string());
                        rl.add_history_entry(line);
                    }

                    if line == "!" {
                        self.is_command_mode = !self.is_command_mode;
                        if self.is_command_mode {
                            println!("Entered Linux command mode. Type 'quit' to exit.");
                        } else {
                            println!("Exited Linux command mode.");
                        }
                        continue;
                    }

                    if self.is_command_mode {
                        if line == "quit" {
                            self.is_command_mode = false;
                            println!("Exited Linux command mode.");
                            continue;
                        }

                        match self.execute_command(line) {
                            Ok(output) => {
                                println!("{}", output);
                                let interaction = format!("Executed command: {}\nOutput: {}", line, output);
                                self.add_to_recent_interactions(interaction.clone());
                                self.update_context(&interaction, "");
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
}

fn load_config() -> Result<Config> {
    let exe_path = env::current_exe().context("Failed to get executable path")?;
    let exe_dir = exe_path.parent().context("Failed to get executable directory")?;
    let config_path = exe_dir.join("config.yml");

    let config_str = std::fs::read_to_string(&config_path)
        .context("Failed to read config.yml")?;
    let config: Config = serde_yaml::from_str(&config_str)
        .context("Failed to parse config.yml")?;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config()?;
    let mut assistant = LinuxCommandAssistant::new(config)?;
    assistant.run().await
}
