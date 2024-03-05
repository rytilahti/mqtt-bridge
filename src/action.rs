use std::fmt;
use std::time::Instant;

use gethostname::gethostname;
use log::{debug, info, warn};
use rumqttc::{AsyncClient, QoS};
use serde::{Deserialize, Serialize};
use serde_json::{self};
use shlex::{self};
use tokio::process::Command;

use crate::Config;

#[derive(Serialize, Deserialize, Debug)]
struct DeviceInfo {
    name: String,
    identifiers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct DiscoveryInfo {
    name: String,
    unique_id: String,
    command_topic: String,
    device: DeviceInfo,
    availability_topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_press: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_category: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Action {
    name: String,
    command: String,
    icon: Option<String>,
    #[serde(skip)]
    instance_name: String,
    #[serde(skip)]
    availability_topic: String,
}

impl Action {
    /// Execute the command
    pub async fn execute(&self) {
        info!("Executing {}", self);
        debug!("Executing command: {}", self.command);
        let start = Instant::now();

        let splitted = shlex::split(self.command.as_str()).unwrap();
        let cmd = &splitted[0];

        let mut child = Command::new(cmd);
        if splitted.len() > 1 {
            let args = &splitted[1..];
            child.args(args);
        }

        match child.output().await {
            Ok(out) => info!("Execution finished: {:?}", out),
            Err(err) => warn!("Failed to execute: {}", err),
        };

        let duration = start.elapsed();
        info!("Executing {} took {:?}", self, duration);
    }

    /// Return topic base for the instance.
    fn topic_base(&self) -> String {
        let base = format!("mqttbridge/{}", self.instance_name);
        base
    }

    /// Return topic we listen for calls of this action.
    pub fn command_topic(&self) -> String {
        let topic = format!("{}/{}/call", self.topic_base(), self.unique_id());
        topic
    }

    /// Return slugified name.
    fn unique_id(&self) -> String {
        self.name.to_lowercase().replace(' ', "_")
    }

    /// Topic that informs homeassistant about its existence.
    fn discovery_topic(&self) -> String {
        let topic = format!(
            "homeassistant/button/{}/{}/config",
            self.instance_name,
            self.unique_id()
        );
        topic
    }

    /// Payload for homeassistant mqtt discovery.
    fn discovery_payload(&self) -> String {
        let name = gethostname().into_string().unwrap();
        let identifiers = [name.clone()].to_vec();
        let info = DiscoveryInfo {
            name: self.name.clone(),
            unique_id: self.unique_id(),
            availability_topic: self.availability_topic.to_string(),
            command_topic: self.command_topic(),
            payload_press: None,
            entity_category: None,
            icon: self.icon.clone(),
            device: DeviceInfo {
                name: format!("mqtt-bridge @ {}", name),
                identifiers,
                model: None,
                manufacturer: None,
            },
        };
        serde_json::to_string(&info).unwrap()
    }

    /// Publish discovery information and subscribe to action topics.
    pub async fn publish_and_subscribe(&mut self, client: &AsyncClient, config: &Config) {
        info!("Initializing {}", self);

        // Initialize the instance_name from config, there must be a better way for this?
        self.instance_name = config.mqtt.instance_name.clone();
        self.availability_topic = config.availability_topic();

        // Subscribe to action topics
        let sub = client
            .subscribe(self.command_topic(), QoS::AtLeastOnce)
            .await;

        match sub {
            Ok(_res) => info!("Subscribed to {} for {}", self.command_topic(), self),
            Err(err) => panic!("Unable to subscribe to {}: {}", self.command, err),
        }

        // Publish discovery info
        let _pub = client
            .publish(
                self.discovery_topic(),
                QoS::AtLeastOnce,
                true,
                self.discovery_payload(),
            )
            .await;

        match _pub {
            Ok(_res) => info!("Published discovery info to {}", self.discovery_topic()),
            Err(err) => panic!("Unable to publish to {}: {}", self.discovery_topic(), err),
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<Action {}>", self.name)
    }
}
