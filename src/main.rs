use api::Command;
use axum::{routing::post, Json, Router};
use color_eyre::{
    eyre::{eyre, WrapErr},
    Result,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

mod api {
    use color_eyre::{eyre::eyre, Result};
    use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
    use serde::Serialize;

    const DISCORD_URL: &str = "https://discord.com/api/v10";

    pub struct Client {
        inner: reqwest::Client,
    }

    impl Client {
        pub fn new(bot_key: &str) -> Result<Self> {
            let mut header_map = HeaderMap::new();
            let mut bot_token_header_value = HeaderValue::from_str(&format!("Bot {}", bot_key))?;
            bot_token_header_value.set_sensitive(true);
            header_map.insert(AUTHORIZATION, bot_token_header_value);

            let inner = reqwest::ClientBuilder::new()
                .default_headers(header_map)
                .build()?;

            Ok(Self { inner })
        }

        pub async fn set_commands(&self, application_id: &str, commands: &[Command]) -> Result<()> {
            let response = self
                .inner
                .put(format!(
                    "{DISCORD_URL}/applications/{application_id}/commands"
                ))
                .json(commands)
                .send()
                .await?;

            if !response.status().is_success() {
                Err(eyre!(response.text().await?))
            } else {
                Ok(())
            }
        }

        pub async fn set_interaction_endpoints_url(&self, url: &str) -> Result<()> {
            let response = self
                .inner
                .patch(format!("{DISCORD_URL}/applications/@me"))
                .json(&serde_json::json!({
                    "interactions_endpoint_url": url
                }))
                .send()
                .await?;

            if !response.status().is_success() {
                Err(eyre!(response.text().await?))
            } else {
                Ok(())
            }
        }
    }

    #[derive(Serialize)]
    pub struct Command {
        pub name: &'static str,
        pub description: &'static str,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let bot_key =
        std::env::var("BOT_KEY").wrap_err(eyre!("must specify bot key with BOT_KEY env var!"))?;
    let application_id = std::env::var("APPLICATION_ID")
        .wrap_err(eyre!("must specify application ID with APPLICATION_ID"))?;

    let discord_client = api::Client::new(&bot_key)?;

    discord_client
        .set_commands(
            &application_id,
            &[Command {
                name: "city",
                description: "generate a random city",
            }],
        )
        .await?;

    tokio::spawn(async move {
        discord_client
            .set_interaction_endpoints_url("https://rude-bars-sort.loca.lt")
            .await
            .unwrap()
    });

    let app = Router::new().route("/", post(handle));
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[axum::debug_handler]
async fn handle(Json(interaction): Json<Interaction>) -> Json<InteractionResponse> {
    println!("{:?}", interaction);
    let response = InteractionResponse::Pong {
        _type: InteractionCallbackType,
    };
    println!("{}", serde_json::to_string(&response).unwrap());
    Json(response)
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Interaction {
    Ping {
        #[serde(rename = "type")]
        _type: InteractionType<1>,
    },
}

#[derive(Debug)]
struct InteractionType<const T: u8>;

impl<'de, const T: u8> Deserialize<'de> for InteractionType<T> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        if value == T {
            Ok(InteractionType::<T>)
        } else {
            Err(serde::de::Error::custom(eyre!("wrong version!")))
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum InteractionResponse {
    Pong {
        #[serde(rename = "type")]
        _type: InteractionCallbackType<1>,
    },
}

#[derive(Debug)]
struct InteractionCallbackType<const T: u8>;

impl<const T: u8> Serialize for InteractionCallbackType<T> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(T)
    }
}
