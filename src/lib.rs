use std::env;

use discord_flows::{
    http::{Http, HttpBuilder},
    model::Message,
    ProvidedBot,
};
use dotenv::dotenv;
use flowsnet_platform_sdk::logger;

#[no_mangle]
#[tokio::main(flavor = "current_thread")]
pub async fn run() {
    dotenv().ok();
    logger::init();

    let token = env::var("discord_token").unwrap();

    let bot = ProvidedBot::new(&token);
    let discord = HttpBuilder::new(token).build();

    bot.listen(|msg| handler(msg, discord)).await;
}

async fn handler(msg: Message, discord: Http) {
    if msg.author.bot {
        log::info!("ignored bot message");
        return;
    }

    let content = msg.content;
    let channel_id = msg.channel_id;

    _ = discord
        .send_message(
            channel_id.into(),
            &serde_json::json!({
                "content": content
            }),
        )
        .await;
}
