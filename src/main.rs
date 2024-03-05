use action::Action;

use clap::{command, Parser};
use gethostname::gethostname;
use log::{debug, info, LevelFilter};
use rumqttc::{AsyncClient, Event, EventLoop, Incoming, LastWill, MqttOptions, QoS};
use serde::{Deserialize, Serialize};
use serde_yaml::{self};

use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::process;
use std::time::Duration;

mod action;

#[derive(Serialize, Deserialize, Debug)]
struct MqttConfig {
    host: String,
    username: String,
    password: String,
    #[serde(default = "get_hostname")]
    instance_name: String,
}

// Current hostname as a string.
fn get_hostname() -> String {
    gethostname().into_string().unwrap()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    mqtt: MqttConfig,
    actions: Vec<Action>,
}

impl Config {
    fn availability_topic(&self) -> String {
        format!("mqttbridge/{}/available", self.mqtt.instance_name)
    }
}

/// Return path for default configuration file.
fn get_default_config() -> PathBuf {
    let mut conf = dirs::config_dir().unwrap();
    conf.push("mqttbridge.yaml");
    conf
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// "mqtt-bridge" -- execute predefined shell commands on incoming MQTT messages
struct Args {
    /// Configuration file
    #[arg(short, long, default_value = get_default_config().into_os_string() )]
    config: PathBuf,

    /// Debug level, more is more
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

/// Perform mqtt connection establishment and setup for availability.
async fn initialize_mqtt(config: &Config) -> (AsyncClient, EventLoop) {
    let mut opts = MqttOptions::new(
        format!("mqttbridge-{}", process::id()),
        config.mqtt.host.clone(),
        1883,
    );
    opts.set_keep_alive(Duration::from_secs(5));

    // TODO: handle optional username&password
    opts.set_credentials(config.mqtt.username.clone(), config.mqtt.password.clone());

    // Set last will for the availability topic.
    opts.set_last_will(LastWill {
        topic: config.availability_topic(),
        message: "offline".into(),
        qos: QoS::AtLeastOnce,
        retain: true,
    });

    let (client, eventloop) = AsyncClient::new(opts, 10);

    // TODO: handle errors
    let _ = client
        .publish(
            config.availability_topic(),
            QoS::AtLeastOnce,
            true,
            "online".to_string(),
        )
        .await;

    info!(
        "Initialized client, availability topic: {}",
        config.availability_topic()
    );

    (client, eventloop)
}

/// Initialize configured actions and return a (topic, action) map.
async fn initialize_actions(
    client: &AsyncClient,
    config: &Config,
) -> HashMap<std::string::String, Action> {
    // Create tasks for action pub&sub

    // TODO: there is probably a better to allow action to modify itself?
    let mut actions = config.actions.to_owned();
    let tasks: Vec<_> = actions
        .iter_mut()
        .map(|act| act.publish_and_subscribe(client, config))
        .collect();

    // Wait them to finish before starting to poll the event loop
    for task in tasks {
        task.await;
    }

    // Create map (topic=>action) for lookups
    let command_topic_to_action: HashMap<String, Action> = actions
        .into_iter()
        .map(|v| (v.command_topic(), v))
        .collect();

    command_topic_to_action
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let log_level = if args.debug > 0 {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    env_logger::Builder::new().filter_level(log_level).init();

    if !args.config.exists() {
        panic!("No config file found");
    }
    let f = std::fs::File::open(args.config).expect("Could not open file.");
    let config: Config = serde_yaml::from_reader(f).unwrap();
    debug!("Config: {:#?}", &config);

    let (client, mut eventloop) = initialize_mqtt(&config).await;
    let topic_to_action = initialize_actions(&client, &config).await;

    info!("Init done, starting the listening loop.");
    loop {
        while let Ok(notification) = eventloop.poll().await {
            // We are interested only in the incoming pubs
            if let Event::Incoming(Incoming::Publish(packet)) = notification {
                debug!("Received on {}: {:?}", packet.topic, &packet.payload);

                // Should be fine to unwrap w/o checking, as we are subscribed only to our own topics
                // Clone is needed to allow passing copy of the action object to the spawn
                let action = topic_to_action.get(&packet.topic).unwrap().clone();

                // Spawn execute the wanted action,
                // async move {} block is necessary to let the ownership change
                tokio::spawn(async move {
                    action.execute().await;
                });
            }
        }
    }
}
