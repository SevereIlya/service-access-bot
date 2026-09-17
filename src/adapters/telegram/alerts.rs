//! Алерты. Все события уровня ERROR из tracing уезжают в Telegram-чат админу.
//!
//! Поток: [`AdminAlertLayer`] (в tracing) нормализует событие в [`AlertEvent`] и кладёт в unbounded-канал.
//! Воркер [`start_admin_alerter`] читает канал, гасит повторы и отправляет сообщение.
//!
//! Канал заодно работает буфером на старте: события, случившиеся до готовности [`teloxide::Bot`],
//! копятся и отправляются сразу после запуска задачи.

use crate::adapters::telegram::{build_alert_view, BotState};
use chrono::{DateTime, Utc};
use std::{
    collections::HashMap,
    fmt::Debug,
    time::{Duration, Instant},
    collections::hash_map::Entry,
};
use teloxide::{prelude::*, types::ParseMode::Html};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{
    Event, Level, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    EnvFilter, Layer,
    layer::{Context, SubscriberExt},
    util::SubscriberInitExt,
};

/// Подавляем повторы одного и того же места ошибки на это время.
pub(crate) const DEDUP_COOLDOWN: Duration = Duration::from_mins(10);

/// Пауза между отправками, чтобы не упереться в rate limit Telegram при залпе ошибок.
const SEND_PAUSE: Duration = Duration::from_millis(100);

/// ERROR событие для отправки в чат
pub struct AlertEvent {
    pub(crate) message: String,
    pub(crate) target: String,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<u32>,
    pub(crate) fields: Vec<(String, String)>,
    pub(crate) timestamp: DateTime<Utc>,
}

impl AlertEvent {
    /// Для идентификации места ошибки. Либо `файл:строка`, либо `target`.
    fn fingerprint(&self) -> String {
        match (&self.file, self.line) {
            (Some(file), Some(line)) => format!("{file}:{line}"),
            _ => self.target.clone(),
        }
    }
}

/// Преобразует `tracing::Event` в `String` для `AlertEvent`.
#[derive(Default)]
struct AlertVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl AlertVisitor {
    fn push(&mut self, name: &str, value: String) {
        if name == "message" {
            self.message = value;
        } else {
            self.fields.push((name.to_string(), value));
        }
    }
}

/// Интерфейс, через который `tracing` передает данные события.
impl Visit for AlertVisitor {
    /// Для дебаг-маркера, например `user = ?User` -> "User { id: 777, name: "Chmo" }"
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.push(field.name(), format!("{value:?}"));
    }

    /// Обычная строка, например `username` = "@chmo" -> `field.name()` = "username"; `value` = "@chmo"
    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field.name(), value.to_string());
    }
}

/// Слой tracing, собирающий ERROR
pub struct AdminAlertLayer {
    tx: UnboundedSender<AlertEvent>,
    self_target: &'static str,
}

impl AdminAlertLayer {
    #[must_use]
    pub const fn new(tx: UnboundedSender<AlertEvent>) -> Self {
        Self {
            tx,
            self_target: module_path!(),
        }
    }
}

impl<S: Subscriber> Layer<S> for AdminAlertLayer {
    /// Дергается при любом `info!`, `warn!`, `error!` и т.п.
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        if *meta.level() != Level::ERROR || meta.target().starts_with(self.self_target) {
            return;
        }

        let mut visitor = AlertVisitor::default();
        event.record(&mut visitor);

        let _ = self.tx.send(AlertEvent {
            message: visitor.message,
            target: meta.target().to_string(),
            file: meta.file().map(str::to_string),
            line: meta.line(),
            fields: visitor.fields,
            timestamp: Utc::now(),
        });
    }
}

/// Инициализирует fmt в терминал + error алерты в tg.
/// Возвращает приёмник канала алертов.
#[must_use]
pub fn init_tracing_with_alerts() -> UnboundedReceiver<AlertEvent> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(AdminAlertLayer::new(tx))
        .with(filter)
        .init();
    rx
}

/// Воркер доставки алертов. Читает канал и шлёт ошибки в чат админа.
pub async fn start_admin_alerter(
    bot: Bot,
    state: BotState,
    mut rx: UnboundedReceiver<AlertEvent>,
) {
    let chat_id = ChatId(state.admin_chat_id);
    // Ключ: fingerprint; Значени: (когда отправляли, сколько подавили с тех пор)
    // Например: "src/payment/service.rs:57"; (18:10:00, 7)
    let mut dedup: HashMap<String, (Instant, usize)> = HashMap::new();

    while let Some(alert) = rx.recv().await {
        let now = Instant::now();

        let suppressed: usize = match dedup.entry(alert.fingerprint()) {
            Entry::Occupied(mut entry) => {
                let (last_sent, pending) = entry.get_mut();
                // Проверяка. Внутри ли мы окна дедупликации
                if now.duration_since(*last_sent) < DEDUP_COOLDOWN {
                    *pending += 1;
                    continue
                }
                // Сбрасываем счетчик pending и забираем значение в sent. Для usize default() = 0
                let sent = std::mem::take(pending);
                // теперь, например sent = 7, pending = 0
                *last_sent = now;
                sent
            }
            Entry::Vacant(entry) => {
                entry.insert((now, 0));
                0
            }
        };

        let text = build_alert_view(&state.ui_text, &alert, suppressed);

        if let Err(e) = bot.send_message(chat_id, text).parse_mode(Html).await {
            // error! породил бы алерт об алерте.
            tracing::warn!(error = ?e, "Не удалось доставить алерт админу");
        }

        tokio::time::sleep(SEND_PAUSE).await;
    }
}
