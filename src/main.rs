use api::Command;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use color_eyre::{
    eyre::{eyre, WrapErr},
    Result,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand::seq::SliceRandom;
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
    let bot_key = std::env::var("BOT_KEY").wrap_err(eyre!("must specify bot key with BOT_KEY"))?;
    let application_id = std::env::var("APPLICATION_ID")
        .wrap_err(eyre!("must specify application ID with APPLICATION_ID"))?;
    let interaction_endpoints_url = std::env::var("INTERACTION_ENDPOINTS_URL").ok();
    let public_key =
        std::env::var("PUBLIC_KEY").wrap_err(eyre!("must specify public key with PUBLIC_KEY"))?;
    let verifying_key = {
        let bytes = parse_hex(&public_key).ok_or(eyre!("invalid hex"))?;
        VerifyingKey::from_bytes(&bytes)?
    };

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

    if let Some(url) = interaction_endpoints_url {
        tokio::spawn(async move {
            discord_client
                .set_interaction_endpoints_url(&url)
                .await
                .unwrap()
        });
    }

    let app = Router::new()
        .route("/", post(handle))
        .with_state(verifying_key);
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[axum::debug_handler]
async fn handle(
    State(verifying_key): State<VerifyingKey>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<InteractionResponse>, (StatusCode, &'static str)> {
    let signature = headers
        .get("X-Signature-Ed25519")
        .ok_or((StatusCode::BAD_REQUEST, "expected signature key"))?
        .to_str()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "expected signature key to be valid string",
            )
        })?;
    let timestamp = headers
        .get("X-Signature-Timestamp")
        .ok_or((StatusCode::BAD_REQUEST, "expected signature timestamp"))?
        .to_str()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "expected signature timestamp to be valid string",
            )
        })?;

    verify_discord_message(verifying_key, signature, timestamp, &body)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid!!!"))?;

    let interaction: Interaction = serde_json::from_slice(&body)
        .map_err(|_| (StatusCode::BAD_REQUEST, "failed to parse interaction"))?;

    let response = match interaction {
        Interaction::Ping { .. } => InteractionResponse::Pong {
            _type: InteractionCallbackType,
        },
        Interaction::ApplicationCommand { data, .. } => match data.name.as_str() {
            "city" => InteractionResponse::ChannelMessageWithSource {
                _type: InteractionCallbackType,
                data: Message {
                    content: {
                        let cities: Vec<_> = include_str!("city.txt").lines().collect();
                        (*cities.choose(&mut rand::thread_rng()).unwrap()).to_owned()
                    },
                },
            },
            _ => return Err((StatusCode::BAD_REQUEST, "unknown command")),
        },
    };
    Ok(Json(response))
}

pub fn verify_discord_message(
    public_key: VerifyingKey,
    signature: &str,
    timestamp: &str,
    body: &[u8],
) -> Result<()> {
    let signature_bytes = parse_hex(signature).ok_or(eyre!("invalid hex"))?;
    let signature = Signature::from_bytes(&signature_bytes);

    // Format the data to verify (Timestamp + body)
    let msg = [timestamp.as_bytes(), body].concat();

    public_key.verify(&msg, &signature)?;

    Ok(())
}

fn parse_hex<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != N * 2 {
        return None;
    }

    let mut res = [0; N];
    for (i, byte) in res.iter_mut().enumerate() {
        *byte = u8::from_str_radix(s.get(2 * i..2 * (i + 1))?, 16).ok()?;
    }
    Some(res)
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Interaction {
    Ping {
        #[serde(rename = "type")]
        _type: InteractionType<1>,
    },
    ApplicationCommand {
        #[serde(rename = "type")]
        _type: InteractionType<2>,
        data: ApplicationCommandData,
    },
}

#[derive(Debug, Deserialize)]
struct ApplicationCommandData {
    name: String,
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
    ChannelMessageWithSource {
        #[serde(rename = "type")]
        _type: InteractionCallbackType<4>,
        data: Message,
    },
}

#[derive(Debug, Serialize)]
struct Message {
    content: String,
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
