use eliza::Eliza;
use futures::stream::StreamExt;
use image::{ImageBuffer, ImageOutputFormat, RgbImage};
use std::{error::Error, sync::Arc};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, Event};
use twilight_http::Client as HttpClient;
use twilight_model::gateway::Intents;
use twilight_model::http::attachment::Attachment;

use std::sync::Mutex;
use std::time::*;

mod jstd;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token = include_str!("token.txt");

    // Use intents to only receive guild message events.

    // A cluster is a manager for multiple shards that by default
    // creates as many shards as Discord recommends.
    let (cluster, mut events) = Cluster::new(
        token.to_owned(),
        Intents::GUILD_MESSAGES
            | Intents::DIRECT_MESSAGES
            | Intents::MESSAGE_CONTENT
            | Intents::GUILD_INVITES,
    )
    .await?;
    let cluster = Arc::new(cluster);

    // Start up the cluster.
    let cluster_spawn = Arc::clone(&cluster);

    // Start all shards in the cluster in the background.
    tokio::spawn(async move {
        cluster_spawn.up().await;
    });

    // HTTP is separate from the gateway, so create a new client.
    let http = Arc::new(HttpClient::new(token.to_owned()));

    let eliza = Arc::new(Mutex::new(
        Eliza::from_str(include_str!("script.json")).unwrap(),
    ));
    let last_message = Arc::new(Mutex::new(Instant::now()));

    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();

    // Since we only care about new messages, make the cache only
    // cache new messages.
    let cache = InMemoryCache::builder()
        .resource_types(ResourceType::MESSAGE)
        .build();

    // Process each event as they come in.
    while let Some((shard_id, event)) = events.next().await {
        // Update the cache with the event.
        cache.update(&event);

        tokio::spawn(handle_event(
            shard_id,
            event,
            Arc::clone(&http),
            Arc::clone(&eliza),
            Arc::clone(&last_message),
        ));
    }

    Ok(())
}

#[derive(Copy, Clone)]
pub struct SharedImage<'a>(&'a Arc<Mutex<Option<RgbImage>>>);

async fn handle_event(
    shard_id: u64,
    event: Event,
    http: Arc<HttpClient>,
    eliza: Arc<Mutex<Eliza>>,
    last_message: Arc<Mutex<Instant>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::MessageCreate(msg) => {
            match msg.content.as_str() {
                "!ping" => {
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap();
                    http.create_message(msg.channel_id)
                        .content(&format!(
                            "Latency: {:?}",
                            Duration::from_micros(
                                (now.as_micros() - msg.timestamp.as_micros() as u128) as u64
                            )
                        ))?
                        .exec()
                        .await?;
                }
                "!kill" => {
                    std::process::exit(1);
                }
                content => {
                    println!("Message: {}", content);
                    // Make sure that eval can only be used by me. You may replace this with your
                    // own account ID.
                    if content.starts_with("!eval") && msg.author.id.get() == 617536235417763863 {
                        // check the attatchments
                        let script = if msg.attachments.len() > 0 {
                            let file = &msg.attachments[0];
                            // now we fetch it
                            let body = reqwest::get(&file.url).await?.text().await?;
                            body
                        } else {
                            let mut script = String::from(content);
                            script.truncate(script.len() - 3);
                            let script = script.split_off("!eval ```js".len());
                            script
                        };

                        let result = tokio::task::spawn_blocking(move || {
                            let isolate = &mut v8::Isolate::new(Default::default());
                            let scope = &mut v8::HandleScope::new(isolate);
                            let global = v8::ObjectTemplate::new(scope);

                            jstd::populate_global(&global, scope);

                            let context = v8::Context::new_from_template(scope, global);
                            let scope = &mut v8::ContextScope::new(scope, context);
                            let mut scope = v8::TryCatch::new(scope);
                            let filename = v8::String::new(&mut scope, "temp.js").unwrap();
                            let undefined = v8::undefined(&mut scope);

                            let code = v8::String::new(&mut scope, &script).unwrap();
                            let origin = v8::ScriptOrigin::new(
                                &mut scope,
                                filename.into(),
                                0,
                                0,
                                false,
                                0,
                                undefined.into(),
                                false,
                                false,
                                false,
                            );
                            let script = v8::Script::compile(&mut scope, code, Some(&origin));
                            match script {
                                Some(script) => {
                                    let result = script.run(&mut scope);
                                    if let Some(result) = result {
                                        let result = result.to_string(&mut scope).unwrap();

                                        (result.to_rust_string_lossy(&mut scope), {
                                            jstd::IMAGE.with(|f| {
                                                if let Some(ref image) = *f.borrow_mut() {
                                                    let mut encoded =
                                                        std::io::Cursor::new(Vec::new());
                                                    image
                                                        .write_to(
                                                            &mut encoded,
                                                            ImageOutputFormat::Png,
                                                        )
                                                        .unwrap();
                                                    let encoded = encoded.into_inner();
                                                    Some(encoded)
                                                } else {
                                                    None
                                                }
                                            })
                                        })
                                    } else {
                                        (jstd::get_exception(scope), None)
                                    }
                                }
                                None => (jstd::get_exception(scope), None),
                            }
                        })
                        .await?;

                        let formatted = result.0;
                        let formatted = format!("```{}```", formatted);
                        let mut builder = http
                            .create_message(msg.channel_id)
                            .content(&formatted)?
                            .reply(msg.id);

                        if let Some(data) = result.1 {
                            // make an attatchment
                            let filename = "unknown.png".to_owned();
                            let id = 1;

                            let mut attachment = Attachment::from_bytes(filename, data, id);
                            attachment.description("Data extracted from JS script".to_owned());
                            let list = [attachment];
                            builder = builder.attachments(&list).unwrap();
                            builder.exec().await?;
                        } else {
                            builder.exec().await?; // hacky :(
                        }

                        return Ok(());
                    }
                    if msg.author.bot != true
                        && (last_message.lock().unwrap().elapsed() > Duration::from_secs(60)
                            || msg
                                .mentions
                                .iter()
                                .any(|mention| mention.name == "Yabladärgh'"))
                    {
                        *last_message.lock().unwrap() = Instant::now();
                        let response = eliza.lock().unwrap().respond(content);
                        http.create_typing_trigger(msg.channel_id).exec().await?;
                        tokio::time::sleep(Duration::from_millis(6000)).await;
                        http.create_message(msg.channel_id)
                            .content(&response)?
                            .reply(msg.id)
                            .exec()
                            .await?;
                    }
                }
            }
        }
        Event::ShardConnected(_) => {
            println!("Connected on shard {shard_id}");
        }
        // Other events here...
        _ => {}
    }

    Ok(())
}
