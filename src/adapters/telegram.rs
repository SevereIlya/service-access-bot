use crate::application::usecases::commands::register_user::RegisterUserCommand;
use crate::application::usecases::commands::start_trial::StartTrialCommand;
use crate::application::usecases::queries::get_menu_state::GetMenuStateQuery;
use crate::application::usecases::queries::get_user::GetUserQuery;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use crate::adapters::telegram::ui::UiText;

pub mod callbacks;
pub mod commands;
pub mod error;
pub mod handlers;
pub mod router;
pub mod views;
pub mod ui;

#[derive(Clone, Debug)]
pub enum UserState {
    WaitingForTicketText,
    WaitingForTicketReply(i64),
    WaitingForTicketAppend(i64),
}

#[derive(Clone)]
pub struct BotState {
    // Юзкейсы
    pub register_user_cmd: Arc<RegisterUserCommand>,
    pub start_trial_cmd: Arc<StartTrialCommand>,
    pub get_user_query: Arc<GetUserQuery>,
    pub get_menu_state_query: Arc<GetMenuStateQuery>,

    // UI-состояние Телеграма
    pub bot_username: String,
    pub ui: Arc<UiText>,
    pub user_states: Arc<Mutex<HashMap<i64, UserState>>>,
    pub broadcasting_admins: Arc<Mutex<HashSet<i64>>>,

    // Конфиги
    pub admin_chat_id: i64,
}
