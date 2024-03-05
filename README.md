> [!CAUTION]
> This project was made as a quick "hello world" project to learn a bit about rust and its ecosystem.
> Not aimed for production use, but feel free to use and experiment [without guarantees](https://fi.wiktionary.org/wiki/per%C3%A4valotakuu).

## "mqtt-bridge" -- execute predefined shell commands on incoming MQTT messages

- **What:** a simple daemon written in rust, that executes pre-defined shell commands when MQTT topic gets a message.
- **Why:** to allow home automation systems like Home Assistant to trigger some actions on your computer, e.g., turning off the display when the lights are turned off.

All exposed commands are also advertised using the [Home Assistant's MQTT discovery protocol](https://www.home-assistant.io/integrations/mqtt#mqtt-discovery) as buttons,
making them accessible out-of-the-box without any further configuration.

![Screenshot](screenshot.png)

### Usage

```
$ mqtt-bridge --help

"mqtt-bridge" -- execute predefined shell commands on incoming MQTT messages

Usage: mqtt-bridge [OPTIONS]

Options:
  -c, --config <CONFIG>  Configuration file [default: config.yaml]
  -d, --debug...         Debug level, more is more
  -h, --help             Print help
  -V, --version          Print version
```

### Configuration

The actions defined in the configuration file are made callable under MQTT topics following the format `mqttbridge/<instance_name>/<slugified_action_name>/call`.
With the following configuration, sending a message to `mqttbridge/moin/sleep_some/call` will execute the `sleep` command.

```
mqtt:
  host: 192.0.2.123
  username: admin
  password: nimda
  instance_name: moin
actions:
  - name: Turn screen off
    icon: mdi:power
    command: /usr/bin/dbus-send --session --print-reply --dest=org.kde.kglobalaccel /component/org_kde_powerdevil org.kde.kglobalaccel.Component.invokeShortcut string:'Turn Off Screen'
  - name: Sleep some
    command: /usr/bin/sleep 10
```

> [!NOTE]
> If `instance_name` is not defined in the configuration, hostname is used instead.

### systemd config

Modify `mqttbridge.service` to contain the correct path to the compiled binary, link or copy to the unit directory, and start it.

```
ln -s $(realpath mqttbridge.service) ~/.config/systemd/user/
systemctl --user start mqttbridge
systemctl --user enable mqttbridge
```

### Used libraries

This project leverages several external crates, including:

- `runmqttc` for mqtt connectivity
- `log`, `env_logger` for logging
- `dirs` for locating the config
- `tokio` for asyncio
- `clap` for cli arg handling
- `serde`, `serde_{json,yaml}` for serialization and deserialization
- `gethostname` to use hostname as a unique id
- `shlex` for parsing the commands

### TODO

- [ ] Error handling and fault tolerance (broker failures, invalid creds, general failures, ..)
- [ ] [Tests](https://rust-cli.github.io/book/tutorial/testing.html)
- [ ] [Package](https://rust-cli.github.io/book/tutorial/packaging.html) and publish?

### Links

Some resources that I found useful while hacking this together, in no specific order.

- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example)
- [Carefully exploring Rust as a Python developer](https://karimjedda.com/carefully-exploring-rust/)
- [py2rs - From Python into Rust](https://rochacbruno.github.io/py2rs/)
- [hamatti's learning rust series](https://hamatti.org/posts/learning-rust-pattern-matching/)
- [Command line apps in Rust](https://rust-cli.github.io/book/)
