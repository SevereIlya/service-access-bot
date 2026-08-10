pub mod commands;
pub mod queries;

use crate::application::usecases::commands::register_user::RegisterUserCommand;
use crate::application::usecases::commands::start_trial::StartTrialCommand;
use crate::application::usecases::queries::get_menu_state::GetMenuStateQuery;
use crate::application::usecases::queries::get_user::GetUserQuery;
use crate::domain::subscription::DynSubscriptionRepository;
use crate::domain::uow::DynUnitOfWork;
use crate::domain::user::{DynUserRepository, Money};
use std::sync::Arc;
use uuid::Uuid;

pub struct UseCases {
    pub register_user: Arc<RegisterUserCommand>,
    pub start_trial: Arc<StartTrialCommand>,
    pub get_user: Arc<GetUserQuery>,
    pub get_menu_state: Arc<GetMenuStateQuery>,
}

impl UseCases {
    pub fn new(
        user_repo: DynUserRepository,
        subscription_repo: DynSubscriptionRepository,
        uow: DynUnitOfWork,
        uuid_namespace: Uuid,
        base_price: Money,
    ) -> Self {
        // === Commands ===
        let register_user = Arc::new(RegisterUserCommand::new(
            user_repo.clone(),
            uuid_namespace,
            base_price,
        ));
        let start_trial = Arc::new(StartTrialCommand::new(uow));

        // === Querys ===
        let get_user = Arc::new(GetUserQuery::new(user_repo));
        let get_menu_state = Arc::new(GetMenuStateQuery::new(subscription_repo));

        Self {
            register_user,
            start_trial,
            get_user,
            get_menu_state,
        }
    }
}
