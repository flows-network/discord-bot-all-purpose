use std::{ env, str };
use async_openai::{
    types::{
        ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client as OpenAIClient,
    config::Config,

};
use base64::{ engine::general_purpose, Engine };
use serde_json::json;
use std::collections::HashMap;
use cloud_vision_flows::text_detection;
use discord_flows::{ model::{ Attachment, Message }, ProvidedBot, Bot, message_handler };
use flowsnet_platform_sdk::logger;
use store_flows as store;
use secrecy::Secret;
use reqwest::header::{ HeaderValue, HeaderMap, CONTENT_TYPE, USER_AGENT };

#[no_mangle]
#[tokio::main(flavor = "current_thread")]
pub async fn on_deploy() {
    let token = std::env::var("discord_token").unwrap();
    let bot = ProvidedBot::new(token);
    bot.listen_to_messages().await;
}

#[message_handler]
async fn handler(msg: Message) {
    logger::init();
    let model = env::var("LLM_MODEL").unwrap_or("llama-chat-7b".to_string());
    let discord_token = env::var("discord_token").unwrap();
    let bot_id = std::env::var("bot_id").unwrap().parse::<u64>().unwrap();
    let placeholder_text = env::var("placeholder").unwrap_or("Typing ...".to_string());
    let help_msg = env
        ::var("help_msg")
        .unwrap_or(
            "You can enter text or upload an image with text to chat with this bot. The bot can take several different assistant roles. Type command /qa or /translate or /summarize or /medical or /code or /reply_tweet to start.".to_string()
        );

    let bot = ProvidedBot::new(discord_token);
    let discord = bot.get_client();

    if msg.author.bot {
        log::info!("ignored bot message");
        return;
    }
    if msg.member.is_some() {
        let mut mentions_me = false;
        for u in &msg.mentions {
            log::debug!("The user ID is {}", u.id.as_u64());
            if *u.id.as_u64() == bot_id {
                mentions_me = true;
                break;
            }
        }
        if !mentions_me {
            log::debug!("ignored guild message");
            return;
        }
    }
    let channel_id = msg.channel_id;

    match msg.content.as_str() {
        "/help" => {
            _ = discord.send_message(
                channel_id.into(),
                &serde_json::json!({
                    "content": &help_msg
                })
            ).await;
            return;
        }
        "/start" => {
            _ = discord.send_message(
                channel_id.into(),
                &serde_json::json!({
                    "content": &help_msg
                })
            ).await;
            store::set(&channel_id.to_string(), json!(true), None);
            store::set(
                &format!("{}:system_prompt", channel_id),
                json!("You are a helpful assistant answering questions on Discord."),
                None
            );
            log::info!("Started QA converstion for {}", channel_id);
            return;
        }
        "/qa" => {
            _ = discord.send_message(
                channel_id.into(),
                &serde_json::json!({
                    "content": "I am ready for general QA"
                })
            ).await;
            store::set(&channel_id.to_string(), json!(true), None);
            store::set(
                &format!("{}:system_prompt", channel_id),
                json!("You are a helpful assistant answering questions on Discord."),
                None
            );
            log::info!("Started QA converstion for {}", channel_id);
            return;
        }
        "/summarize" => {
            _ = discord.send_message(
                channel_id.into(),
                &serde_json::json!({
                    "content": "I am ready to summarize text"
                })
            ).await;
            store::set(&channel_id.to_string(), json!(true), None);
            store::set(
                &format!("{}:system_prompt", channel_id),
                json!(
                    "You are a helpful assistant. Please summarize the next message in short bullet points. Please always answer in English even if the original text is not English."
                ),
                None
            );
            log::info!("Started summarization for {}", channel_id);
            return;
        }
        "/code" => {
            _ = discord.send_message(
                channel_id.into(),
                &serde_json::json!({
                    "content": "I am ready to review source code"
                })
            ).await;
            store::set(&channel_id.to_string(), json!(true), None);
            store::set(
                &format!("{}:system_prompt", channel_id),
                json!(
                    "You are an experienced software developer. Please review the computer source code in the next message, explain what it does, and identify potential problems. Please also make suggestions on how to improve it."
                ),
                None
            );
            log::info!("Started code review for {}", channel_id);
            return;
        }
        "/medical" => {
            _ = discord.send_message(
                channel_id.into(),
                &serde_json::json!({
                    "content": "I am ready to review and summarize doctor notes or medical test results"
                })
            ).await;
            store::set(&channel_id.to_string(), json!(true), None);
            store::set(
                &format!("{}:system_prompt", channel_id),
                json!(
                    "You are a medical doctor, you'll read a lab report and tell the user the most important findings of the report in short bullets, please use the following template: The major findings are:\n 1) [the name of the measurement] [status of the reading]\n ... \n one sentence summary about the subject's health status."
                ),
                None
            );
            log::info!("Started medical review for {}", channel_id);
            return;
        }
        "/translate" => {
            _ = discord.send_message(
                channel_id.into(),
                &serde_json::json!({
                    "content": "I am ready to translate anything into English"
                })
            ).await;
            store::set(&channel_id.to_string(), json!(true), None);
            store::set(
                &format!("{}:system_prompt", channel_id),
                json!(
                    "You are an English language translator. For every message you receive, please translate it to English. Please respond with just the English translation and nothing more. If the input message is already in English, please fix any grammar errors and improve the writing."
                ),
                None
            );
            log::info!("Started English translation for {}", channel_id);
            return;
        }
        "/reply_tweet" => {
            _ = discord.send_message(
                channel_id.into(),
                &serde_json::json!({
                    "content": "I am ready to reply a tweet for you"
                })
            ).await;
            store::set(&channel_id.to_string(), json!(true), None);
            store::set(
                &format!("{}:system_prompt", channel_id),
                json!(
                    "You are a social media marketing expert. You will receive the text from a tweet. Please generate 3 clever replies to it. Then follow user suggestions to improve the reply tweets."
                ),
                None
            );
            log::info!("Started Twitter marketer for {}", channel_id);
            return;
        }

        text => {
            let placeholder = discord
                .send_message(
                    channel_id.into(),
                    &serde_json::json!({
                    "content": &placeholder_text
                })
                ).await
                .unwrap();

            let restart = store
                ::get(&channel_id.to_string())
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if restart {
                log::info!("Detected restart = true");
                store::set(&channel_id.to_string(), json!(false), None);
            }

            let system_prompt = store
                ::get(&format!("{}:system_prompt", channel_id))
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();

            let mut question = text.to_string();
            if question.is_empty() {
                log::info!("received empty text");
                let urls = get_image_urls(msg.attachments);
                if urls.is_empty() {
                    log::info!("no image urls");
                    log::debug!("The input message is neither a text nor and image");
                    _ = discord.edit_message(
                        channel_id.into(),
                        placeholder.id.into(),
                        &serde_json::json!({
                            "content": "Sorry, I cannot understand your message. Can you try again?"
                        })
                    ).await;
                    return;
                }

                for url in urls {
                    log::debug!("Try to DOWNLOAD {}", &url);
                    let bs64 = match download_image(url) {
                        Ok(b) => b,
                        Err(e) => {
                            log::warn!("{}", e);
                            _ = discord.edit_message(
                                channel_id.into(),
                                placeholder.id.into(),
                                &serde_json::json!({
                                    "content": "There is a problem with the uploaded file. Can you try again?"
                                })
                            ).await;
                            continue;
                        }
                    };
                    log::debug!("Downloaded size {}", bs64.len());
                    let detected = match text_detection(bs64) {
                        Ok(t) => {
                            log::debug!("text_detection: {}", t);
                            t
                        }
                        Err(e) => {
                            log::debug!("The input image does not contain text: {}", e);
                            _ = discord.edit_message(
                                channel_id.into(),
                                placeholder.id.into(),
                                &serde_json::json!({
                                    "content": "Sorry, the input image does not contain text. Can you try again"
                                })
                            ).await;
                            continue;
                        }
                    };

                    question.push_str(&detected);
                    question.push_str("\n");
                }
            }

            log::info!("Ask question: {}", question);

            match chat_inner_async(&system_prompt, &question, 512, &model).await {
                Ok(r) => {
                    log::info!("Answer: {}", r);
                    let resps = sub_strings(&r, 1800);

                    _ = discord.edit_message(
                        channel_id.into(),
                        placeholder.id.into(),
                        &serde_json::json!({
                            "content": resps[0]
                        })
                    ).await;

                    if resps.len() > 1 {
                        for resp in resps.iter().skip(1) {
                            _ = discord.send_message(
                                channel_id.into(),
                                &serde_json::json!({
                                    "content": resp
                                })
                            ).await;
                        }
                    }
                }
                Err(e) => {
                    log::error!("LLM returns error: {}", e);
                    _ = discord.edit_message(
                        channel_id.into(),
                        placeholder.id.into(),
                        &serde_json::json!({
                            "content": "Sorry an error has occurred with OpenAI"
                        })
                    ).await;
                    log::error!("OpenAI returns error: {}", e);
                    return;
                }
            }
        }
    }
}

fn get_image_urls(attachments: Vec<Attachment>) -> Vec<String> {
    attachments
        .iter()
        .filter_map(|a| {
            match a.content_type.as_ref() {
                Some(ct) if ct.starts_with("image") => Some(a.url.clone()),
                _ => None,
            }
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
                Err(
                    format!(
                        "response failed: {}, body: {}",
                        r.reason(),
                        String::from_utf8_lossy(&writer)
                    )
                )
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

fn sub_strings(string: &str, sub_len: usize) -> Vec<&str> {
    let mut subs = Vec::with_capacity(string.len() / sub_len);
    let mut iter = string.chars();
    let mut pos = 0;

    while pos < string.len() {
        let mut len = 0;
        for ch in iter.by_ref().take(sub_len) {
            len += ch.len_utf8();
        }
        subs.push(&string[pos..pos + len]);
        pos += len;
    }
    subs
}

pub async fn chat_inner_async(
    system_prompt: &str,
    user_input: &str,
    max_token: u16,
    model: &str
) -> anyhow::Result<String> {
    let api_key = env::var("LLM_API_KEY").expect("LLM_API_KEY-must-be-set");
    let api_base = env::var("LLM_API_BASE").unwrap_or(String::from("http://52.37.228.1:8080/v1"));
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(USER_AGENT, HeaderValue::from_static("MyClient/1.0.0"));
    let config = LocalServiceProviderConfig {
        api_base: api_base,
        headers: headers,
        api_key: Secret::new(api_key),
        query: HashMap::new(),
    };

    let client = OpenAIClient::with_config(config);
    let messages = vec![
        ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt)
            .build()
            .expect("Failed to build system message")
            .into(),
        ChatCompletionRequestUserMessageArgs::default().content(user_input).build()?.into()
    ];
    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(max_token)
        .model(model)
        .messages(messages)
        .build()?;

    match client.chat().create(request).await {
        Ok(chat) =>
            match chat.choices[0].message.clone().content {
                Some(res) => {
                    // log::info!("{:?}", chat.choices[0].message.clone());
                    Ok(res)
                }
                None => Err(anyhow::anyhow!("Failed to get reply from OpenAI")),
            }
        Err(_e) => {
            log::error!("Error getting response from OpenAI: {:?}", _e);
            Err(anyhow::anyhow!(_e))
        }
    }
}

#[derive(Clone, Debug)]
pub struct LocalServiceProviderConfig {
    pub api_base: String,
    pub headers: HeaderMap,
    pub api_key: Secret<String>,
    pub query: HashMap<String, String>,
}

impl Config for LocalServiceProviderConfig {
    fn headers(&self) -> HeaderMap {
        self.headers.clone()
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.api_base, path)
    }

    fn query(&self) -> Vec<(&str, &str)> {
        self.query
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    fn api_base(&self) -> &str {
        &self.api_base
    }

    fn api_key(&self) -> &Secret<String> {
        &self.api_key
    }
}
