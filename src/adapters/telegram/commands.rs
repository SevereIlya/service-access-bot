use teloxide::macros::BotCommands;

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "lowercase", description = "Доступные команды:")]
pub enum Command {
    #[command(description = "Запустить бота и открыть меню")]
    Start(String),

    #[command(hide, description = "Панель администратора")]
    Admin,
}