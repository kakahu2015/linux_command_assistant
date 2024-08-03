use anyhow::{Context, Result};
use reqwest::Client;
use rustyline::Editor;
use rustyline::history::DefaultHistory;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::process::Command;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
mod completer;
use completer::LinuxCommandCompleter;

#[derive(Debug, Deserialize)]
struct Config {
    openai: OpenAIConfig,
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
    command_history: DefaultHistory, // 使用 DefaultHistory 替代 History
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
    fn new(config: Config) -> Self {
        let client = Client::new();
        let context = vec![Message {
            role: "system".to_string(),
            content: "You are a Linux command assistant. Your primary function is to help users with Linux commands, explain their usage, and provide examples. Always prioritize giving clear, concise, and accurate information about Linux commands and operations.".to_string(),
        }];
        Self {
            config,
            client,
            context,
            recent_interactions: VecDeque::with_capacity(5),
            command_history: DefaultHistory::new(), // 使用 DefaultHistory::new()
        }
    }

    /////////////////////////////
     async fn get_ai_response(&mut self, prompt: &str) -> Result<String> {
        let mut messages = self.context.clone();
        if !self.recent_interactions.is_empty() {
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

        let request = ChatCompletionRequest {
            model: self.config.openai.model.clone(),
            messages,
        };

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", self.config.openai.api_key))?);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let response = self.client.post(&self.config.openai.api_base)
            .headers(headers)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to OpenAI API")?;

        let status = response.status();
        let body = response.text().await.context("Failed to read response body")?;

        if !status.is_success() {
            return Err(anyhow::anyhow!("API request failed with status {}: {}", status, body));
        }

        let response: ChatCompletionResponse = serde_json::from_str(&body)
            .context("Failed to parse API response")?;

        if let Some(choice) = response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("No response content from AI"))
        }
    }
    /////////////////////////////////////////////////////////////

    fn execute_command(&self, command: &str) -> Result<String> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .context("Failed to execute command")?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
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
        if self.context.len() > 10 {
            self.context.drain(1..3);
        }
    }

    fn add_to_recent_interactions(&mut self, interaction: String) {
        self.recent_interactions.push_back(interaction);
        if self.recent_interactions.len() > 5 {
            self.recent_interactions.pop_front();
        }
    }
///////////////////////////run start/////////////////////////
async fn run(&mut self) -> Result<()> {
    println!("Welcome to Linux Command Assistant.");
    println!("Type 'exit' to quit. Use '!' prefix to execute local Linux commands.");
    println!("Ask me anything about Linux commands!");

    //let config = Config::builder().history_ignore_space(true).build();
    let mut rl = Editor::<LinuxCommandCompleter, DefaultHistory>::new()?;
    //let mut rl = Editor::with_config(config)?;
    rl.set_helper(Some(LinuxCommandCompleter));

    loop {
        let readline = rl.readline("linux-assistant> ");
        match readline {
            Ok(line) => {
                if line.eq_ignore_ascii_case("exit") {
                    break;
                }

                if line.starts_with('!') {
                    let command = &line[1..];
                    if !rl.add_history_entry(line.as_str()) {
                        println!("Warning: Failed to add command to history.");
                    }
                    match self.execute_command(command) {
                        Ok(output) => {
                            println!("Command output:\n{}", output);
                            self.add_to_recent_interactions(format!("Command: {}\nOutput: {}", command, output));
                        }
                        Err(e) => println!("Error executing command: {}", e),
                    }
                } else {
                    // ... 处理 AI 响应的代码保持不变 ...
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
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
}

fn load_config() -> Result<Config> {
    let config_str = std::fs::read_to_string("config.yml")
        .context("Failed to read config.yml")?;
    let config: Config = serde_yaml::from_str(&config_str)
        .context("Failed to parse config.yml")?;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config()?;
    let mut assistant = LinuxCommandAssistant::new(config);
    assistant.run().await
}
