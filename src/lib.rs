use std::{env, str};

use base64::{engine::general_purpose, Engine};
use dotenv::dotenv;
use serde_json::json;

use cloud_vision_flows::text_detection;
use discord_flows::{
    http::{Http, HttpBuilder},
    model::{Attachment, ChannelId, Message, MessageId},
    ProvidedBot,
};
use flowsnet_platform_sdk::logger;
use openai_flows::{
    chat::{ChatModel, ChatOptions},
    OpenAIFlows,
};
use store_flows as store;

struct App {
    discord: Http,
    openai: OpenAIFlows,
}

#[no_mangle]
#[tokio::main(flavor = "current_thread")]
pub async fn run() {
    dotenv().ok();
    logger::init();

    let token = env::var("discord_token").unwrap();
    let placeholder_text = env::var("placeholder").unwrap_or("Typing ...".to_string());
    let help_msg = env::var("help_msg").unwrap_or("You can enter text or upload an image with text to chat with this bot. The bot can take several different assistant roles. Type command /qa or /translate or /summarize or /code or /reply_tweet to start.".to_string());

    let bot = ProvidedBot::new(&token);
    let discord = HttpBuilder::new(token).build();

    let mut openai = OpenAIFlows::new();
    openai.set_retry_times(3);

    let app = App { discord, openai };

    // used to debug wasmedge
    println!();
    bot.listen(|msg| app.handle(msg, help_msg, placeholder_text))
        .await;
}

impl App {
    async fn handle(&self, msg: Message, help_msg: String, placeholder_text: String) {
        if msg.author.bot {
            log::info!("ignored bot message");
            return;
        }

        if msg.member.is_some() {
            log::info!("ignored guild message");
            return;
        }

        let chat_id = msg.id.to_string();
        let content = msg.content;
        let channel_id = msg.channel_id;

        match content.as_str() {
            "/help" => {
                self.send_msg(channel_id, help_msg).await;
            }
            "/start" => {
                self.send_msg(channel_id, help_msg).await;
                store::set(&chat_id.to_string(), json!(true), None);
                store::set(
                    &format!("{}:system_prompt", chat_id),
                    json!("You are a helpful assistant answering questions on Telegram."),
                    None,
                );
                log::info!("Started QA converstion for {}", chat_id);
            }
            "/qa" => {
                self.send_msg(channel_id, "I am ready for general QA").await;
                store::set(&chat_id.to_string(), json!(true), None);
                store::set(
                    &format!("{}:system_prompt", chat_id),
                    json!("You are a helpful assistant answering questions on Telegram."),
                    None,
                );
                log::info!("Started QA converstion for {}", chat_id);
            }
            "/summarize" => {
                self.send_msg(channel_id, "I am ready to summarize text")
                    .await;
                store::set(&chat_id, json!(true), None);
                store::set(&format!("{}:system_prompt", chat_id), json!("You are a helpful assistant. Please summarize the next message in short bullet points. Please always answer in English even if the original text is not English."), None);
                log::info!("Started Chinese translation for {}", chat_id);
            }
            "/code" => {
                self.send_msg(channel_id, "I am ready to review source code")
                    .await;
                store::set(&chat_id, json!(true), None);
                store::set(&format!("{}:system_prompt", chat_id), json!("You are an experienced software developer. Please review the computer source code in the next message, explain what it does, and identify potential problems. Please also make suggestions on how to improve it."), None);
                log::info!("Started code review for {}", chat_id);
            }
            "/translate" => {
                self.send_msg(channel_id, "I am ready to translate anything into English")
                    .await;
                store::set(&chat_id, json!(true), None);
                store::set(&format!("{}:system_prompt", chat_id), json!("You are an English language translator. For every message you receive, please translate it to English. Please respond with just the English translation and nothing more. If the input message is already in English, please fix any grammar errors and improve the writing."), None);
                log::info!("Started English translation for {}", chat_id);
            }
            "/reply_tweet" => {
                self.send_msg(channel_id, "I am ready to reply a tweet for you")
                    .await;
                store::set(&chat_id, json!(true), None);
                store::set(&format!("{}:system_prompt", chat_id), json!("You are a social media marketing expert. You will receive the text from a tweet. Please generate 3 clever replies to it. Then follow user suggestions to improve the reply tweets."), None);
                log::info!("Started Twitter marketer for {}", chat_id);
            }
            text => {
                let placeholder = self.send_msg(channel_id, placeholder_text).await.unwrap();

                let restart = store::get(&chat_id)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if restart {
                    log::info!("Detected restart = true");
                    store::set(&chat_id, json!(false), None);
                }

                let system_prompt = store::get(&format!("{}:system_prompt", chat_id))
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();

                let co = ChatOptions {
                    // model: ChatModel::GPT4,
                    model: ChatModel::GPT35Turbo,
                    restart,
                    system_prompt: Some(system_prompt.as_str()),
                };

                if text.is_empty() {
                    let urls = get_image_urls(msg.attachments);

                    if urls.is_empty() {
                        log::debug!("The input message is neither a text nor and image");
                        self.send_msg(
                            channel_id,
                            "Sorry, I cannot understand your message. Can you try again?",
                        )
                        .await;

                        return;
                    }

                    for url in urls {
                        let bs64 = match download_image(url) {
                            Ok(b) => b,
                            Err(e) => {
                                log::warn!("{}", e);
                                continue;
                            }
                        };
                        let detected = match text_detection(bs64) {
                            Ok(t) => {
                                log::debug!("text_detection: {}", t);
                                t
                            }
                            Err(e) => {
                                log::debug!("The input image does not contain text: {}", e);
                                self.send_msg(channel_id, "Sorry, the input image does not contain text. Can you try again").await;
                                continue;
                            }
                        };

                        self.chat(&chat_id, &detected, &co, channel_id, placeholder.id)
                            .await;
                    }
                } else {
                    self.chat(&chat_id, text, &co, channel_id, placeholder.id)
                        .await;
                }
            }
        }
    }
}

impl App {
    async fn chat(
        &self,
        chat_id: &str,
        text: &str,
        co: &ChatOptions<'_>,
        channel_id: ChannelId,
        message_id: MessageId,
    ) {
        match self.openai.chat_completion(chat_id, text, co).await {
            Ok(r) => {
                self.edit_msg(channel_id, message_id, r.choice).await;
            }
            Err(e) => {
                self.edit_msg(
                    channel_id,
                    message_id,
                    "Sorry, an error has occured. Please try again later!",
                )
                .await;
                log::error!("OpenAI returns error: {}", e);
            }
        }
    }
}

impl App {
    async fn send_msg<S: AsRef<str>>(&self, channel_id: ChannelId, content: S) -> Option<Message> {
        let res = self
            .discord
            .send_message(
                channel_id.into(),
                &serde_json::json!({
                    "content": content.as_ref()
                }),
            )
            .await;

        res.map_err(|e| log::error!("failed to send message to discord: {}", e))
            .ok()
    }

    async fn edit_msg<S: AsRef<str>>(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        content: S,
    ) -> Option<Message> {
        let res = self
            .discord
            .edit_message(
                channel_id.into(),
                message_id.into(),
                &serde_json::json!({
                    "content": content.as_ref()
                }),
            )
            .await;
        res.map_err(|e| log::error!("failed to send message to discord: {}", e))
            .ok()
    }
}

fn get_image_urls(attachments: Vec<Attachment>) -> Vec<String> {
    attachments
        .iter()
        .filter_map(|a| match a.content_type.as_ref() {
            Some(ct) if ct.starts_with("image") => Some(a.url.clone()),
            _ => None,
        })
        .collect()
}

fn download_image(url: String) -> Result<String, String> {
    let mut writer = Vec::new();
    let resp = http_req::request::get(url, &mut writer);

    match resp {
        Ok(r) => {
            if r.status_code().is_success() {
                Ok(general_purpose::STANDARD.encode(writer))
            } else {
                Err(format!(
                    "response failed: {}, body: {}",
                    r.reason(),
                    String::from_utf8_lossy(&writer)
                ))
            }
        }
        Err(e) => Err(e.to_string()),
    }
}
