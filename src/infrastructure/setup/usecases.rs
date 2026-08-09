use crate::application::usecases::commands::register_user::RegisterUserCommand;
use crate::application::usecases::commands::start_trial::StartTrialCommand;
use crate::application::usecases::queries::get_menu_state::GetMenuStateQuery;
use crate::application::usecases::queries::get_user::GetUserQuery;
use crate::domain::user::Money;
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::setup::repo::Repositories;
use std::sync::Arc;
use tracing::debug;

pub struct UseCases {
    pub register_user: Arc<RegisterUserCommand>,
    pub start_trial: Arc<StartTrialCommand>,
    pub get_user: Arc<GetUserQuery>,
    pub get_menu_state: Arc<GetMenuStateQuery>,
}

impl UseCases {
    pub fn new(repos: &Repositories, config: &AppConfig) -> Self {
        debug!("Сборка юзкейсов...");

        // === Commands ===
        let register_user = Arc::new(RegisterUserCommand::new(
            repos.user.clone(),
            config.vpn.uuid_namespace,
            Money(config.payments.base_price),
        ));
        let start_trial = Arc::new(StartTrialCommand::new(repos.uow.clone()));

        // === Querys ===
        let get_user = Arc::new(GetUserQuery::new(repos.user.clone()));
        let get_menu_state = Arc::new(GetMenuStateQuery::new(repos.subscription.clone()));

        Self {
            register_user,
            start_trial,
            get_user,
            get_menu_state,
        }
    }
}
