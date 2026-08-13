pub mod commands;
pub mod queries;

use crate::application::usecases::commands::expire_lapsed_subscriptions::ExpireLapsedSubscriptionsCommand;
use crate::application::usecases::commands::register_user::RegisterUserCommand;
use crate::application::usecases::commands::send_expiry_warnings::SendExpiryWarningsCommand;
use crate::application::usecases::commands::start_trial::StartTrialCommand;
use crate::application::usecases::queries::get_menu_state::GetMenuStateQuery;
use crate::application::usecases::queries::get_user::GetUserQuery;
use crate::domain::notification::DynNotifier;
use crate::domain::subscription::DynSubscriptionRepository;
use crate::domain::uow::DynUnitOfWork;
use crate::domain::user::{DynUserRepository, Money};
use crate::domain::vpn::DynVpnAccessRevoker;
use std::sync::Arc;
use uuid::Uuid;

pub struct UseCases {
    pub register_user: Arc<RegisterUserCommand>,
    pub start_trial: Arc<StartTrialCommand>,
    pub expire_lapsed_subscriptions: Arc<ExpireLapsedSubscriptionsCommand>,
    pub send_expiry_warnings: Arc<SendExpiryWarningsCommand>,

    pub get_user: Arc<GetUserQuery>,
    pub get_menu_state: Arc<GetMenuStateQuery>,
}

impl UseCases {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_repo: DynUserRepository,
        subscription_repo: DynSubscriptionRepository,
        uow: DynUnitOfWork,
        vpn_revoker: DynVpnAccessRevoker,
        notifier: DynNotifier,
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
        let expire_lapsed_subscriptions =
            Arc::new(ExpireLapsedSubscriptionsCommand::new(
                subscription_repo.clone(),
                user_repo.clone(),
                vpn_revoker,
                notifier.clone(),
            ));
        let send_expiry_warnings = Arc::new(SendExpiryWarningsCommand::new(
            subscription_repo.clone(),
            user_repo.clone(),
            notifier,
        ));

        // === Querys ===
        let get_user = Arc::new(GetUserQuery::new(user_repo));
        let get_menu_state = Arc::new(GetMenuStateQuery::new(subscription_repo));

        Self {
            register_user,
            start_trial,
            expire_lapsed_subscriptions,
            send_expiry_warnings,
            get_user,
            get_menu_state,
        }
    }
}
