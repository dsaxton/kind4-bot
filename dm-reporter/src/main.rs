use chrono::{Local, NaiveDateTime};
use log::{error, info, LevelFilter};
use nostr::prelude::ToBech32;
use nostr::{EventBuilder, Filter, Keys, Kind, Metadata, Tag, Timestamp, Url};
use nostr_sdk::bitcoin::secp256k1::SecretKey;
use nostr_sdk::relay::pool::RelayPoolNotification::Event;
use nostr_sdk::Client;
use std::io::Write;
use std::str::FromStr;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    client
        .add_relay("wss://universe.nostrich.land", None)
        .await?;
    client
        .add_relay("wss://nostr-pub.wellorder.net", None)
        .await?;

    loop {
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
                    let sender_npub = event.pubkey.to_bech32().unwrap();

                    info!("Posting to API endpoint...");
                    let http_client = reqwest::Client::new();
                    match http_client.put(API_URL).body(json_event).send().await {
                        Ok(_) => info!("Successfully stored event."),
                        Err(err) => error!("{}", err),
                    }

                    let mut receiver_npub = "".to_string();
                    for tag in event.tags.iter() {
                        if let Tag::PubKey(pubkey, _) = tag {
                            receiver_npub = pubkey.to_bech32().unwrap_or("".to_owned());
                            break;
                        }
                    }

                    let current_unix_timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let current_datetime = NaiveDateTime::from_timestamp_micros(current_unix_timestamp as i64 *  1000000)
                        .unwrap()
                        .to_string();
                    let one_week_ago_unix_timestamp = current_unix_timestamp - 7 * 24 * 60 * 60;
                    let one_week_ago_datetime = NaiveDateTime::from_timestamp_micros(one_week_ago_unix_timestamp as i64 * 1000000)
                        .unwrap()
                        .to_string();

                    if current_unix_timestamp % 100 == 0 {
                        info!("Getting counts since {}...", one_week_ago_datetime);
                        let counts = http_client
                            .get(format!(
                                "{}counts?sender={}&since={}",
                                API_URL, sender_npub, one_week_ago_unix_timestamp
                            ))
                            .send()
                            .await?
                            .json::<std::collections::HashMap<String, u32>>()
                            .await?;

                        let mut message = format!(
                            "Message sent from nostr:{} to nostr:{} at {}. I've seen nostr:{} message the following users since {}:\n",
                            sender_npub, receiver_npub, current_datetime, sender_npub, one_week_ago_datetime
                        );
                        for (key, value) in counts.iter() {
                            message = format!("{}\nnostr:{} {} time(s)", message, key, value);
                        }

                        info!("Publishing note...");
                        info!("{}", message);
                        client.publish_text_note(message, &[]).await?;
                    }
                }
                Ok(())
            })
            .await?;

        thread::sleep(Duration::from_secs(15));
    }
}
