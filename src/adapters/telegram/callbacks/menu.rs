#[derive(Debug, Clone)]
pub enum MenuAction {
    StartTrial,
    MyVpn,
    Profile,
    Tariffs(i32),
    About,
    Referral,
    Help,
    Down,
    Main,
}
impl MenuAction {
    pub(super) fn parse<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Option<Self> {
        match parts.next()? {
            "trial" => Some(Self::StartTrial),
            "myvpn" => Some(Self::MyVpn),
            "profile" => Some(Self::Profile),
            "tariffs" => {
                let devices = parts.next().and_then(|s| s.parse::<i32>().ok()).unwrap_or(1);
                Some(Self::Tariffs(devices))
            }
            "about" => Some(Self::About),
            "referral" => Some(Self::Referral),
            "help" => Some(Self::Help),
            "down" => Some(Self::Down),
            "main" => Some(Self::Main),
            _ => None,
        }
    }
}
