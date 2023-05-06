use chrono::{Local, NaiveDateTime};
use log::{error, info, LevelFilter};
use nostr::prelude::ToBech32;
use nostr::{EventBuilder, Filter, Keys, Kind, Metadata, Tag, Timestamp, Url};
use nostr_sdk::bitcoin::secp256k1::SecretKey;
use nostr_sdk::relay::pool::RelayPoolNotification::Event;
use nostr_sdk::Client;
use std::io::Write;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

const API_URL: &str = "https://kind4-backend.denseresidual5921.workers.dev/";
const PROFILE_IMAGE_URL: &str = "https://nostr.build/i/nostr.build_acd58b907f3b9af0adaf0b0c615ead34cdee0d3c6aa86492f48acd014006d939.jpg";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let keys = if args.len() == 1 {
        Keys::generate()
    } else if args.len() == 2 {
        Keys::new(SecretKey::from_str(&args[1]).expect("Invalid secret key string"))
    } else {
        error!("Too many arguments");
        std::process::exit(1);
    };

    info!("bot pubkey: {}", keys.public_key());
    info!(
        "bot secret: {}",
        keys.secret_key().unwrap().display_secret()
    );
    let client = Client::new(&keys);

    client.add_relay("wss://nostr.wine", None).await?;
    client.add_relay("wss://nos.lol", None).await?;
    client.add_relay("wss://nostr.fmt.wiz.biz", None).await?;
    client.add_relay("wss://nostr.zebedee.cloud", None).await?;
    client.add_relay("wss://relay.damus.io", None).await?;
    client.add_relay("wss://relay.nostr.band", None).await?;
    client.add_relay("wss://bitcoiner.social", None).await?;
    client
        .add_relay("wss://universe.nostrich.land", None)
        .await?;
    client
        .add_relay("wss://nostr-pub.wellorder.net", None)
        .await?;
    client.connect().await;

    let metadata = Metadata::new()
        .name("dm-reporter")
        .picture(Url::parse(PROFILE_IMAGE_URL)?);
    let metadata_event = EventBuilder::set_metadata(metadata).to_event(&keys)?;
    client.send_event(metadata_event).await?;

    let subscription = Filter::new()
        .kinds(vec![Kind::Regular(4)])
        .since(Timestamp::now());
    client.subscribe(vec![subscription]).await;

    client
        .handle_notifications(|notification| async {
            if let Event(relay_url, event) = notification {
                let json_event = event.clone().as_json();
                info!("Received event from relay {}", relay_url);
                info!("{}", json_event.clone());
                let sender_npub = event.pubkey.to_bech32().unwrap();
                let datetime =
                    match NaiveDateTime::from_timestamp_micros(event.created_at.as_i64() * 1000000)
                    {
                        Some(datetime) => datetime.to_string(),
                        None => {
                            error!("Could not parse timestamp as datetime");
                            "".to_string()
                        }
                    };
                let tags = event.tags;
                let content = event.content;
                let mut receiver_npub = "".to_string();
                for tag in tags {
                    if let Tag::PubKey(pubkey, _) = tag {
                        receiver_npub = pubkey.to_bech32().unwrap();
                        break;
                    }
                }

                let http_client = reqwest::Client::new();
                match http_client.put(API_URL).body(json_event).send().await {
                    Ok(_) => info!("Successfully stored event."),
                    Err(err) => error!("{}", err),
                }

                let current_unix_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("Could not get UNIX timestamp").as_secs();
                let one_week_ago = current_unix_timestamp - 7 * 24 * 60 * 60;

                info!("Getting counts since UNIX timestamp {}...", one_week_ago);
                let counts = match http_client
                    .get(format!("{}counts?sender={}&since={}", API_URL, sender_npub, one_week_ago))
                    .send()
                    .await
                {
                    Ok(response) => match response
                        .json::<std::collections::HashMap<String, u32>>()
                        .await
                    {
                        Ok(map) => map,
                        Err(err) => {
                            error!("{}", err);
                            std::collections::HashMap::new()
                        }
                    },
                    Err(err) => {
                        error!("{}", err);
                        std::collections::HashMap::new()
                    }
                };

                info!("Displaying receiver counts for {}...", sender_npub);
                for (key, value) in counts.iter() {
                    info!("npub: {}, count: {}", key, value)
                }

                let count = counts.get(&receiver_npub).unwrap_or(&0);
                let time_string = if *count ==  1_u32 {
                    "time"
                } else {
                    "times"
                };
                let bot_content = format!(
                    "Sender: nostr:{}\nReceiver: nostr:{}\nContent length: {}\nSent at: {}\n\nI've seen nostr:{} message nostr:{} {} {} in the past week.",
                    sender_npub,
                    receiver_npub,
                    content.len(),
                    datetime,
                    sender_npub,
                    receiver_npub,
                    count,
                    time_string,
                );
                info!("Publishing note...");
                info!("{}", bot_content);
                client.publish_text_note(bot_content, &[]).await?;
            }
            Ok(())
        })
        .await?;
    Ok(())
}
