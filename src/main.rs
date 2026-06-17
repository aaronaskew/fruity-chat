use anyhow::Result;
use clap::Parser;
use futures_lite::StreamExt;
use iroh::{Endpoint, endpoint::presets, protocol::Router};
use iroh_gossip::{
    api::{Event, GossipReceiver},
    net::Gossip,
    proto::TopicId,
};
use iroh_services::Client;
use std::{collections::HashMap, str::FromStr};

mod fruit;
mod message;
mod ticket;

use crate::{
    fruit::random_fruit,
    message::{Message, MessageBody},
    ticket::Ticket,
};

/// 🍓 fruity-chat - It's like regular chat but fruitier 🍎
///
/// 🍉 This broadcasts unsigned messages over iroh-gossip. 🥥
///
/// 🍌 By default a new endpoint id is created when starting the example. 🍑
///
/// 🍇 By default, we use the default n0 address lookup services to dial by `EndpointId`. 🥝
#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Set your nickname.
    #[clap(short, long)]
    name: Option<String>,
    /// Set the bind port for our socket. By default, a random port will be used.
    #[clap(short, long, default_value = "0")]
    bind_port: u16,
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    /// Open a chat room for a topic and print a ticket for others to join.
    Open,
    /// Join a chat room from a ticket.
    Join {
        /// The ticket, as base32 string.
        ticket: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // parse the cli command
    let (topic, endpoints) = match &args.command {
        Command::Open => {
            let topic = TopicId::from_bytes(rand::random());
            println!("> opening chat room for topic {topic}");
            (topic, vec![])
        }
        Command::Join { ticket } => {
            let Ticket { topic, endpoints } = Ticket::from_str(ticket)?;
            println!("> joining chat room for topic {topic}");
            (topic, endpoints)
        }
    };

    let endpoint = Endpoint::bind(presets::N0).await?;

    let _services_client = match Client::builder(&endpoint).api_secret_from_env() {
        Ok(builder) => {
            let client = builder.name("ping-receiver")?.build().await?;
            println!("Connected to Iroh Services");
            Some(client)
        }
        Err(_) => None,
    };

    println!("> our endpoint id: {}", endpoint.id());
    let gossip = Gossip::builder().spawn(endpoint.clone());

    let router = Router::builder(endpoint.clone())
        .accept(iroh_gossip::ALPN, gossip.clone())
        .spawn();

    // in our main file, after we create a topic `id`:
    // print a ticket that includes our own endpoint id and endpoint addresses
    let ticket = {
        // Get our address information, includes our
        // `EndpointId`, our `RelayUrl`, and any direct
        // addresses.
        let me = endpoint.addr();
        let endpoints = vec![me];
        Ticket { topic, endpoints }
    };
    println!("> ticket to join us:\n\n{ticket}\n");

    // join the gossip topic by connecting to known endpoints, if any
    let endpoint_ids = endpoints.iter().map(|p| p.id).collect();
    if endpoints.is_empty() {
        println!("> waiting for endpoints to join us...");
    } else {
        println!("> trying to connect to {} endpoints...", endpoints.len());
    };
    let (sender, receiver) = gossip
        .subscribe_and_join(topic, endpoint_ids)
        .await?
        .split();
    println!("> connected!");

    // broadcast our name, if set
    if let Some(name) = args.name {
        let message = Message::new(MessageBody::AboutMe {
            from: endpoint.id(),
            name,
        });
        sender.broadcast(message.to_vec().into()).await?;
    }

    // subscribe and print loop
    tokio::spawn(subscribe_loop(receiver));

    // spawn an input thread that reads stdin
    // create a multi-provider, single-consumer channel
    let (line_tx, mut line_rx) = tokio::sync::mpsc::channel(1);
    // and pass the `sender` portion to the `input_loop`
    std::thread::spawn(move || input_loop(line_tx));

    // broadcast each line we type
    println!("> type a message and hit enter to broadcast...");
    // listen for lines that we have typed to be sent from `stdin`
    while let Some(text) = line_rx.recv().await {
        // fruitify the message first
        let fruit_len = rand::random_range(1..=3);
        let text = format!("{} {}", random_fruit(fruit_len), text,);

        // create a message from the text
        let message = Message::new(MessageBody::Message {
            from: endpoint.id(),
            text: text.clone(),
        });
        // broadcast the encoded message
        sender.broadcast(message.to_vec().into()).await?;
        // print to ourselves the text that we sent
        println!("> sent: {text}");
    }
    router.shutdown().await?;

    Ok(())
}

// Handle incoming events
async fn subscribe_loop(mut receiver: GossipReceiver) -> Result<()> {
    // keep track of the mapping between `EndpointId`s and names
    let mut names = HashMap::new();
    // iterate over all events
    while let Some(event) = receiver.try_next().await? {
        // if the Event is a `GossipEvent::Received`, let's deserialize the message:
        if let Event::Received(msg) = event {
            // deserialize the message and match on the
            // message type:
            match Message::from_bytes(&msg.content)?.body {
                MessageBody::AboutMe { from, name } => {
                    // if it's an `AboutMe` message
                    // add an entry into the map
                    // and print the name
                    names.insert(from, name.clone());
                    println!("> {} is now known as {}", from.fmt_short(), name);
                }
                MessageBody::Message { from, text } => {
                    // if it's a `Message` message,
                    // get the name from the map
                    // and print the message
                    let name = names
                        .get(&from)
                        .map_or_else(|| from.fmt_short().to_string(), String::to_string);
                    println!("{}: {}", name, text);
                }
            }
        }
    }
    Ok(())
}

fn input_loop(line_tx: tokio::sync::mpsc::Sender<String>) -> Result<()> {
    let mut buffer = String::new();
    let stdin = std::io::stdin(); // We get `Stdin` here.
    loop {
        stdin.read_line(&mut buffer)?;
        line_tx.blocking_send(buffer.clone())?;
        buffer.clear();
    }
}
