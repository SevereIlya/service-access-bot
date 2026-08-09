pub use crate::adapters::telegram::handlers::callback::*;

// ============================================================================================== //
//                                      СУЩНОСТИ КОЛЛБЕКОВ
// ============================================================================================== //

#[derive(Debug, Clone)]
pub enum CallbackAction {
    Menu(MenuAction),
    // Purchase(PurchaseAction),
    // Vpn(VpnAction),
    // Admin(AdminAction),
    // Support(SupportAction),
    Ignore,
    Unknown(String),
}

#[derive(Debug, Clone)]
pub enum MenuAction {
    StartTrial,
    Router,
    Profile,
    Tariffs,
    Referral,
    Help,
    Down,
    Main,
}

// #[derive(Debug, Clone)]
// pub enum PurchaseAction {
//     Buy { plan: String, devices: u32 },
// }
//
// #[derive(Debug, Clone)]
// pub enum VpnAction {
//     VlessRealityTCP(VlessRealityTCPAction),
//     WireGuard(WireGuardAction),
// }
//
// #[derive(Debug, Clone)]
// pub enum AdminAction {
//     MainPanel,
//     UsersPage { page: u32 },
//     ViewUser { user_id: i64 },
//     SetDiscount { user_id: i64, discount: u8 },
//     Ban { user_id: i64 },
//     Unban { user_id: i64 },
//     ViewTicket { ticket_id: i64 },
//     ReplyTicket { ticket_id: i64 },
//     CloseTicket { ticket_id: i64 },
//     TicketsList,
//     BroadcastMode,
// }
//
// #[derive(Debug, Clone)]
// pub enum SupportAction {
//     Menu,
//     CreateNew,
//     ListTickets,
//     ViewTicket { ticket_id: i64 },
//     AppendTicket { ticket_id: i64 },
// }
//
// #[derive(Debug, Clone)]
// pub enum VlessRealityTCPAction {
//
// }
//
// #[derive(Debug, Clone)]
// pub enum WireGuardAction {
//     WgView(i64),
//     WgDelete(i64),
// }

// ============================================================================================== //
//                                    ИМПЛЕМЕНТАЦИИ КОЛЛБЕКОВ
// ============================================================================================== //

impl CallbackAction {
    pub fn parse(data: &str) -> Self {
        if data == "ignore" {
            return Self::Ignore;
        }

        let mut parts = data.split(':');

        let domain = parts.next().unwrap_or("");

        match domain {
            "menu" => MenuAction::parse(&mut parts).map(Self::Menu),
            _ => None,
        }
        .unwrap_or_else(|| Self::Unknown(data.to_string()))
    }
}

impl MenuAction {
    fn parse<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Option<Self> {
        match parts.next()? {
            "trial" => Some(Self::StartTrial),
            "router" => Some(Self::Router),
            "profile" => Some(Self::Profile),
            "tariffs" => Some(Self::Tariffs),
            "referral" => Some(Self::Referral),
            "help" => Some(Self::Help),
            "down" => Some(Self::Down),
            "main" => Some(Self::Main),
            _ => None,
        }
    }
}