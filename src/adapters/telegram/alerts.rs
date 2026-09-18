//! Алерты об ошибках приложения в Telegram.
//!
//! [`AdminAlertLayer`] перехватывает события `ERROR` из `tracing`,
//! собирает данные события и его активных спанов и передаёт их через канал.
//! [`start_admin_alerter`] читает канал, подавляет повторяющиеся ошибки и отправляет их в чат администратора.
//!
//! Поля спанов, переданные при их создании, также включаются в алерт.
//! Они собираются от ближайшего к событию спана к корневому.
//!
//! Канал является промежуточным буфером между `tracing` и воркером доставки,
//! поэтому события могут накапливаться до запуска [`start_admin_alerter`].

use crate::adapters::telegram::{BotState, build_alert_view};
use chrono::{DateTime, Utc};
use std::{
    collections::{HashMap, hash_map::Entry},
    fmt::Debug,
    time::{Duration, Instant},
};
use teloxide::{prelude::*, types::ParseMode::Html};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{
    Event, Id, Level, Subscriber,
    field::{Field, Visit},
    span::Attributes,
};
use tracing_subscriber::{
    EnvFilter, Layer,
    layer::{Context, SubscriberExt},
    registry::LookupSpan,
    util::SubscriberInitExt,
};

/// Интервал, в течение которого повторяющийся алерт не отправляется.
pub(crate) const DEDUP_COOLDOWN: Duration = Duration::from_mins(10);

/// Пауза между отправкой алертов в Telegram.
const SEND_PAUSE: Duration = Duration::from_millis(100);

/// Данные ошибки, подготовленные [`AdminAlertLayer`] для отправки администратору.
pub struct AlertEvent {
    pub(crate) message: String,
    pub(crate) target: String,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<u32>,
    pub(crate) fields: Vec<(String, String)>,
    pub(crate) span_fields: Vec<(String, String)>,
    pub(crate) timestamp: DateTime<Utc>,
}

impl AlertEvent {
    /// Возвращает ключ, используемый для подавления повторяющихся ошибок.
    ///
    /// Предпочтительно используется сочетание файла и строки, а при их отсутствии - `target` события.
    fn fingerprint(&self) -> String {
        match (&self.file, self.line) {
            (Some(file), Some(line)) => format!("{file}:{line}"),
            _ => self.target.clone(),
        }
    }
}

/// Собирает значения полей `tracing` в представление, пригодное для [`AlertEvent`].
#[derive(Default)]
struct AlertVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl AlertVisitor {
    /// Добавляет поле в накопленные данные события.
    ///
    /// Поле с именем `message` сохраняется отдельно как основное сообщение,
    /// остальные поля добавляются в список дополнительных данных.
    fn push(&mut self, name: &str, value: String) {
        if name == "message" {
            self.message = value;
        } else {
            self.fields.push((name.to_string(), value));
        }
    }

    /// Возвращает копию накопленных полей.
    ///
    /// Копирование необходимо, поскольку поля спана хранятся в его `extensions`
    /// и должны быть извлечены из них независимо от времени жизни спана.
    fn snapshot(&self) -> Vec<(String, String)> {
        self.fields.clone()
    }
}

/// Преобразует поля событий `tracing` в данные [`AlertEvent`].
impl Visit for AlertVisitor {
    /// Записывает поле, представленное через `Debug`.
    ///
    /// Используется, например, для событий вида `error!(user = ?user, "Ошибка")`.
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.push(field.name(), format!("{value:?}"));
    }

    /// Записывает строковое поле.
    ///
    /// Используется для событий вида `error!(username = "user", "Ошибка")`.
    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field.name(), value.to_string());
    }
}

/// Слой `tracing`, преобразующий события уровня `ERROR` в [`AlertEvent`].
///
/// Слой не отправляет сообщения самостоятельно. Он только собирает данные события
/// и передаёт их через [`UnboundedSender`] воркеру [`start_admin_alerter`].
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

/// Подключает [`AdminAlertLayer`] к произвольному `tracing`-подписчику.
///
/// Ограничение `S: Subscriber` требует от переданного подписчика поддержку базового интерфейса `tracing`.
///
/// `LookupSpan` нужен для доступа к данным активных спанов.
/// Благодаря этому слой может получить текущий стек спанов через [`Context`]
/// и прочитать сохранённые в `extensions` поля.
///
/// `for<'lookup>` означает HRTB: реализация [`LookupSpan`] должна быть доступна для любого времени жизни, с которым `Context` обращается к спанам.
/// В типичной конфигурации `tracing_subscriber::registry()` предоставляет это хранилище, а остальные слои работают поверх него.
impl<S> Layer<S> for AdminAlertLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    /// Сохраняет поля спана в его `extensions`.
    ///
    /// При создании спана `tracing` передаёт его исходные поля через `attrs`.
    /// Они сохраняются, чтобы впоследствии включить их в алерт события, произошедшего внутри этого спана.
    ///
    /// Поля, добавленные позже через `Span::record()`, этим механизмом не обновляются.
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut visitor = AlertVisitor::default();
        attrs.record(&mut visitor);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(visitor);
        }
    }

    /// Обрабатывает каждое событие `tracing`.
    ///
    /// В канал передаются только события уровня `ERROR`.
    /// Для них дополнительно собираются поля всех активных спанов от ближайшего к событию к корневому.
    ///
    /// События самого слоя игнорируются, чтобы ошибка доставки алерта не породила новый алерт и не создала рекурсивный цикл.
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();
        if *meta.level() != Level::ERROR || meta.target().starts_with(self.self_target) {
            return;
        }

        let mut visitor = AlertVisitor::default();
        event.record(&mut visitor);

        let mut span_fields = Vec::new();
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope {
                if let Some(fields) = span.extensions().get::<AlertVisitor>() {
                    span_fields.extend(fields.snapshot());
                }
            }
        }

        let _ = self.tx.send(AlertEvent {
            message: visitor.message,
            target: meta.target().to_string(),
            file: meta.file().map(str::to_string),
            line: meta.line(),
            fields: visitor.fields,
            span_fields,
            timestamp: Utc::now(),
        });
    }
}

/// Инициализирует `tracing` с выводом логов в терминал и сбором событий уровня `ERROR` для последующей отправки в Telegram.
///
/// Возвращает приёмник канала, который должен быть передан в [`start_admin_alerter`].
///
/// Для фильтрации используется [`EnvFilter`] со значением из переменной окружения `RUST_LOG`.
/// Если переменная отсутствует или содержит некорректное значение, используется уровень `info`.
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

/// Запускает воркер доставки алертов в Telegram.
///
/// Воркер читает [`AlertEvent`] из канала,
/// подавляет повторяющиеся события в течение [`DEDUP_COOLDOWN`]
/// и формирует сообщение через [`build_alert_view`].
///
/// Повторные события не отправляются сразу: их количество накапливается и передаётся в следующий алерт после окончания окна подавления.
///
/// Между отправками выдерживается [`SEND_PAUSE`].
pub async fn start_admin_alerter(bot: Bot, state: BotState, mut rx: UnboundedReceiver<AlertEvent>) {
    let chat_id = ChatId(state.admin_chat_id);
    // Ключ: fingerprint; Значени: (когда отправляли, сколько подавили с тех пор)
    // Например: "src/payment/service.rs:57"; (18:10:00, 7)
    let mut dedup: HashMap<String, (Instant, usize)> = HashMap::new();

    while let Some(alert) = rx.recv().await {
        let now = Instant::now();

        let suppressed: usize = match dedup.entry(alert.fingerprint()) {
            Entry::Occupied(mut entry) => {
                let (last_sent, pending) = entry.get_mut();
                if now.duration_since(*last_sent) < DEDUP_COOLDOWN {
                    *pending += 1;
                    continue;
                }
                let suppressed = std::mem::take(pending);
                *last_sent = now;
                suppressed
            }
            Entry::Vacant(entry) => {
                entry.insert((now, 0));
                0
            }
        };

        let text = build_alert_view(&state.ui_text, &alert, suppressed);

        if let Err(e) = bot.send_message(chat_id, text).parse_mode(Html).await {
            // Ошибка не должна сама порождать новый алерт
            tracing::warn!(error = ?e, "Не удалось доставить алерт админу");
        }

        tokio::time::sleep(SEND_PAUSE).await;
    }
}
