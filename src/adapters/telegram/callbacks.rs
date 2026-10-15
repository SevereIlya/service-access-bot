mod menu;
mod purchase;

pub use menu::MenuAction;
pub use purchase::PurchaseAction;

#[derive(Debug, Clone)]
pub enum CallbackAction {
    Menu(MenuAction),
    Purchase(PurchaseAction),
    Ignore,
    Unknown(String),
}

impl CallbackAction {
    pub fn parse(data: &str) -> Self {
        if data == "ignore" {
            return Self::Ignore;
        }
        let mut parts = data.split(':');
        let namespace = parts.next().unwrap_or("");
        match namespace {
            "menu" => MenuAction::parse(&mut parts).map(Self::Menu),
            "purchase" => PurchaseAction::parse(&mut parts).map(Self::Purchase),
            _ => None,
        }
        .unwrap_or_else(|| Self::Unknown(data.to_string()))
    }
}
