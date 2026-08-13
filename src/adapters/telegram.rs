use crate::adapters::telegram::ui::UiText;
use crate::application::usecases::UseCases;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub mod callbacks;
pub mod commands;
pub mod error;
pub mod handlers;
pub mod notifier;
pub mod router;
pub mod ui;
pub mod views;

#[derive(Clone, Debug)]
pub enum UserState {
    WaitingForTicketText,
    WaitingForTicketReply(i64),
    WaitingForTicketAppend(i64),
}

#[derive(Clone)]
pub struct BotState {
    pub usecases: Arc<UseCases>,

    // UI-состояние Телеграма
    pub bot_username: String,
    pub ui: Arc<UiText>,
    pub user_states: Arc<Mutex<HashMap<i64, UserState>>>,
    pub broadcasting_admins: Arc<Mutex<HashSet<i64>>>,

    // Конфиги
    pub admin_chat_id: i64,
}
