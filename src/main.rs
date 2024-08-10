use anyhow::{Context, Result};
use reqwest::Client;
use rustyline::Editor;
use rustyline::config::Config as RustylineConfig;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::process::Command;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
mod completer;
use completer::LinuxCommandCompleter;
use rustyline::error::ReadlineError;
use std::io::{stdout, Write};
use serde_json::json;

const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

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
            content: config.system_prompt.clone(), 
        }];
        let max_recent_interactions = config.max_recent_interactions;
        Self {
            config,
            client,
            context,
            recent_interactions: VecDeque::with_capacity(max_recent_interactions),
            command_history: Vec::new(),
        }
    }

    /////////////////////////////
 async fn get_ai_response(&mut self, prompt: &str) -> Result<String> {
        println!("Entering get_ai_response function");
        
        // 可选：DNS 解析
        /*
        let resolver = AsyncResolver::tokio_from_system_conf().await?;
        let start = Instant::now();
        println!("Resolving DNS for {}", self.config.openai.api_base);
        let ips = resolver.lookup_ip(self.config.openai.api_base.clone()).await?;
        println!("DNS resolved in {:?}. IPs: {:?}", start.elapsed(), ips.iter().collect::<Vec<_>>());
        */

        let mut messages = self.context.clone();
        if messages.is_empty() {
            println!("Context is empty, adding system prompt");
            messages.push(Message {
                role: "system".to_string(),
                content: self.config.system_prompt.clone(),
            });
        }
        
        if !self.recent_interactions.is_empty() {
            println!("Adding recent interactions to context");
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

        println!("Preparing request payload");
        let request = serde_json::json!({
            "model": self.config.openai.model,
            "messages": messages,
        });

        println!("Setting up headers");
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", self.config.openai.api_key))
            .context("Failed to create Authorization header")?);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        println!("Sending request to OpenAI API: {}", self.config.openai.api_base);
        let start = Instant::now();
        let response = self.client.post(&self.config.openai.api_base)
            .headers(headers)
            .json(&request)
            .send()
            .await;

        match response {
            Ok(resp) => {
                println!("Request completed in {:?}", start.elapsed());
                println!("Response status: {}", resp.status());
                println!("Response headers: {:?}", resp.headers());

                let body = resp.text().await.context("Failed to read response body")?;
                println!("Response body: {}", body);

                if resp.status().is_success() {
                    let response: serde_json::Value = serde_json::from_str(&body)
                        .context("Failed to parse API response")?;
                    
                    if let Some(content) = response["choices"][0]["message"]["content"].as_str() {
                        Ok(content.to_string())
                    } else {
                        Err(anyhow::anyhow!("No response content from AI"))
                    }
                } else {
                    Err(anyhow::anyhow!("API request failed with status {}: {}", resp.status(), body))
                }
            },
            Err(e) => {
                println!("Request failed after {:?}", start.elapsed());
                println!("Error details: {:?}", e);
                if let Some(url) = e.url() {
                    println!("Failed URL: {}", url);
                }
                Err(anyhow::anyhow!("Failed to send request: {}", e))
            }
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
         // 只保留最新的 max_openai_context 条消息
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
