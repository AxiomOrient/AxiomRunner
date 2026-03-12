use axonrunner_adapters::{
    AdapterHealth, ChannelAdapter, ChannelMessage, TelegramChannelAdapter, TelegramConfig,
};
#[cfg(feature = "channel-discord")]
use axonrunner_adapters::{DiscordChannelAdapter, DiscordConfig};
#[cfg(feature = "channel-irc")]
use axonrunner_adapters::{IrcChannelAdapter, IrcConfig};
#[cfg(feature = "channel-matrix")]
use axonrunner_adapters::{MatrixChannelAdapter, MatrixConfig};
#[cfg(feature = "channel-slack")]
use axonrunner_adapters::{SlackChannelAdapter, SlackConfig};
#[cfg(feature = "channel-whatsapp")]
use axonrunner_adapters::{WhatsAppChannelAdapter, WhatsAppConfig};
use std::future::Future;

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime should initialize")
        .block_on(future)
}

fn message(topic: &str, body: &str) -> ChannelMessage {
    ChannelMessage::new(topic, body)
}

#[test]
fn channel_external_telegram_health_and_queue_semantics() {
    let config = TelegramConfig::new("tg_demo_token", vec![String::from("alice")])
        .expect("telegram config should be valid");
    let mut channel = TelegramChannelAdapter::new(config);

    assert_eq!(channel.id(), "channel.telegram");
    assert_eq!(channel.health(), AdapterHealth::Healthy);

    let first =
        block_on(channel.send(message("alerts", "first"))).expect("first send should succeed");
    let second =
        block_on(channel.send(message("alerts", "second"))).expect("second send should succeed");
    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 2);

    let drained = block_on(channel.drain()).expect("drain should succeed");
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].body, "first");
    assert_eq!(drained[1].body, "second");
}

#[test]
fn channel_external_telegram_is_unavailable_without_allowlist() {
    let config =
        TelegramConfig::new("tg_demo_token", Vec::new()).expect("telegram config should be valid");
    let channel = TelegramChannelAdapter::new(config);
    assert_eq!(channel.health(), AdapterHealth::Unavailable);
}

#[cfg(feature = "channel-discord")]
#[test]
fn channel_external_discord_health_matrix() {
    let healthy = DiscordChannelAdapter::new(
        DiscordConfig::new(
            "discord_token",
            Some(String::from("guild-1")),
            vec![String::from("alice")],
        )
        .expect("discord config should be valid"),
    );
    assert_eq!(healthy.health(), AdapterHealth::Healthy);

    let degraded = DiscordChannelAdapter::new(
        DiscordConfig::new("discord_token", None, vec![String::from("alice")])
            .expect("discord config should be valid"),
    );
    assert_eq!(degraded.health(), AdapterHealth::Degraded);

    let unavailable = DiscordChannelAdapter::new(
        DiscordConfig::new(
            "invalid_discord_token",
            Some(String::from("guild-1")),
            vec![String::from("alice")],
        )
        .expect("discord config should be valid"),
    );
    assert_eq!(unavailable.health(), AdapterHealth::Unavailable);
}

#[cfg(feature = "channel-slack")]
#[test]
fn channel_external_slack_health_matrix() {
    let healthy = SlackChannelAdapter::new(
        SlackConfig::new(
            "xoxb-demo-token",
            Some(String::from("C123")),
            vec![String::from("alice")],
        )
        .expect("slack config should be valid"),
    );
    assert_eq!(healthy.health(), AdapterHealth::Healthy);

    let degraded = SlackChannelAdapter::new(
        SlackConfig::new("xoxb-demo-token", None, vec![String::from("alice")])
            .expect("slack config should be valid"),
    );
    assert_eq!(degraded.health(), AdapterHealth::Degraded);

    let unavailable = SlackChannelAdapter::new(
        SlackConfig::new(
            "invalid_slack_token",
            Some(String::from("C123")),
            vec![String::from("alice")],
        )
        .expect("slack config should be valid"),
    );
    assert_eq!(unavailable.health(), AdapterHealth::Unavailable);
}

#[cfg(feature = "channel-matrix")]
#[test]
fn channel_external_matrix_health_matrix() {
    let healthy = MatrixChannelAdapter::new(
        MatrixConfig::new(
            "matrix_token",
            Some(String::from("!room:matrix.org")),
            Some(String::from("matrix.org")),
            vec![String::from("alice")],
        )
        .expect("matrix config should be valid"),
    );
    assert_eq!(healthy.health(), AdapterHealth::Healthy);

    let degraded = MatrixChannelAdapter::new(
        MatrixConfig::new(
            "matrix_token",
            None,
            Some(String::from("matrix.org")),
            vec![String::from("alice")],
        )
        .expect("matrix config should be valid"),
    );
    assert_eq!(degraded.health(), AdapterHealth::Degraded);

    let unavailable = MatrixChannelAdapter::new(
        MatrixConfig::new(
            "invalid_matrix_token",
            Some(String::from("!room:matrix.org")),
            Some(String::from("matrix.org")),
            vec![String::from("alice")],
        )
        .expect("matrix config should be valid"),
    );
    assert_eq!(unavailable.health(), AdapterHealth::Unavailable);
}

#[cfg(feature = "channel-whatsapp")]
#[test]
fn channel_external_whatsapp_health_matrix() {
    let healthy = WhatsAppChannelAdapter::new(
        WhatsAppConfig::new(
            "wa_token",
            Some(String::from("123")),
            Some(String::from("456")),
            vec![String::from("alice")],
        )
        .expect("whatsapp config should be valid"),
    );
    assert_eq!(healthy.health(), AdapterHealth::Healthy);

    let degraded = WhatsAppChannelAdapter::new(
        WhatsAppConfig::new(
            "wa_token",
            Some(String::from("123")),
            None,
            vec![String::from("alice")],
        )
        .expect("whatsapp config should be valid"),
    );
    assert_eq!(degraded.health(), AdapterHealth::Degraded);

    let unavailable = WhatsAppChannelAdapter::new(
        WhatsAppConfig::new(
            "invalid_wa_token",
            Some(String::from("123")),
            Some(String::from("456")),
            vec![String::from("alice")],
        )
        .expect("whatsapp config should be valid"),
    );
    assert_eq!(unavailable.health(), AdapterHealth::Unavailable);
}

#[cfg(feature = "channel-irc")]
#[test]
fn channel_external_irc_health_matrix() {
    let healthy = IrcChannelAdapter::new(
        IrcConfig::new(
            "irc.libera.chat:6697",
            Some(String::from("#ops")),
            "axiombot",
            vec![String::from("alice")],
        )
        .expect("irc config should be valid"),
    );
    assert_eq!(healthy.health(), AdapterHealth::Healthy);

    let degraded = IrcChannelAdapter::new(
        IrcConfig::new(
            "irc.libera.chat:6697",
            None,
            "axiombot",
            vec![String::from("alice")],
        )
        .expect("irc config should be valid"),
    );
    assert_eq!(degraded.health(), AdapterHealth::Degraded);

    let unavailable = IrcChannelAdapter::new(
        IrcConfig::new(
            "invalid.server",
            Some(String::from("#ops")),
            "axiombot",
            vec![String::from("alice")],
        )
        .expect("irc config should be valid"),
    );
    assert_eq!(unavailable.health(), AdapterHealth::Unavailable);
}

#[cfg(all(
    feature = "channel-discord",
    feature = "channel-slack",
    feature = "channel-irc",
    feature = "channel-matrix",
    feature = "channel-whatsapp",
))]
#[test]
fn channel_external_trait_object_contract_for_six_providers() {
    let mut channels: Vec<Box<dyn ChannelAdapter>> = vec![
        Box::new(TelegramChannelAdapter::new(
            TelegramConfig::new("tg_token", vec![String::from("alice")])
                .expect("telegram config should be valid"),
        )),
        Box::new(DiscordChannelAdapter::new(
            DiscordConfig::new(
                "discord_token",
                Some(String::from("guild")),
                vec![String::from("alice")],
            )
            .expect("discord config should be valid"),
        )),
        Box::new(SlackChannelAdapter::new(
            SlackConfig::new(
                "xoxb-slack-token",
                Some(String::from("C123")),
                vec![String::from("alice")],
            )
            .expect("slack config should be valid"),
        )),
        Box::new(MatrixChannelAdapter::new(
            MatrixConfig::new(
                "matrix_token",
                Some(String::from("!room:matrix.org")),
                Some(String::from("matrix.org")),
                vec![String::from("alice")],
            )
            .expect("matrix config should be valid"),
        )),
        Box::new(WhatsAppChannelAdapter::new(
            WhatsAppConfig::new(
                "wa_token",
                Some(String::from("123")),
                Some(String::from("456")),
                vec![String::from("alice")],
            )
            .expect("whatsapp config should be valid"),
        )),
        Box::new(IrcChannelAdapter::new(
            IrcConfig::new(
                "irc.libera.chat:6697",
                Some(String::from("#ops")),
                "axiombot",
                vec![String::from("alice")],
            )
            .expect("irc config should be valid"),
        )),
    ];

    for channel in &mut channels {
        let health = channel.health();
        assert!(matches!(
            health,
            AdapterHealth::Healthy | AdapterHealth::Degraded
        ));

        let receipt = block_on(channel.send(message("ops", "ping"))).expect("send should succeed");
        assert_eq!(receipt.sequence, 1);
        assert!(receipt.accepted);

        let drained = block_on(channel.drain()).expect("drain should succeed");
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].body, "ping");
    }
}
