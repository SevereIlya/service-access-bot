use anyhow::anyhow;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::{Client, RequestBuilder, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use tracing::info;

/// Текст ошибки GORM (Go-бэкенд 3x-ui), означающий отсутствие записи в БД панели.
///
/// # Important
///
/// Проверено на 3x-ui v3.7.0. Логика определения отсутствующего клиента в [`Self::upsert`] зависит от этого сообщения.
/// При обновлении панели необходимо проверить актуальность текста ошибки, если авторегистрация перестанет корректно определять отсутствие клиента.
const ERR_RECORD_NOT_FOUND: &str = "record not found";

/// Стандартный ответ панели 3x-ui.
#[derive(Debug, Deserialize)]
struct XuiResponse {
    success: bool,
    msg: Option<String>,
    obj: Option<Value>,
}

/// Конфигурация клиента 3x-ui.
///
/// # Особенности API 3x-ui (v3.7.0+)
///
/// - Поле `id` в запросе на создание клиента используется панелью как UUID и сохраняется в БД в колонке `uuid`. Поле `uuid` отдельно передавать не следует.
/// - Поле `auth` используется Hysteria2, а `password` — Trojan/Shadowsocks. Клиент использует один и тот же UUID для `id`, `auth` и `password`.
/// - `limit_ip` и `expiry_time` устанавливаются в `0`. Управление сроком действия клиента выполняется на стороне бота через `enable`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientObj {
    pub id: String,
    pub email: String,
    pub sub_id: String,
    // uuid не указываем
    pub password: String,
    pub auth: String,
    pub flow: String,
    pub limit_ip: i32,
    pub limit_hwid: i32,
    #[serde(rename = "totalGB")]
    pub total_gb: i64,
    pub expiry_time: i64,
    pub enable: bool,
    pub tg_id: i64,
    pub comment: String,
}

#[derive(Debug)]
pub struct XuiClient {
    base_url: String,
    client: Client,
}

impl XuiClient {
    /// Создаёт API-клиент панели.
    ///
    /// Клиент создается с указанным базовым URL и API-токеном.
    /// Токен автоматически добавляется ко всем запросам в заголовке `Authorization` в формате `Bearer <token>`.
    ///
    /// Для HTTP-клиента установлен таймаут запроса в 10 секунд.
    /// Проверка TLS-сертификатов отключена, поэтому допускаются самоподписанные и иные невалидные сертификаты.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если API-токен невозможно преобразовать в значение HTTP-заголовка или если не удалось создать HTTP-клиент.
    pub fn new(base_url: String, api_token: &str) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();
        let auth_value = HeaderValue::from_str(&format!("Bearer {api_token}"))
            .map_err(|e| anyhow!("Некорректный API-токен: {e}"))?;
        headers.insert(AUTHORIZATION, auth_value);

        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self { base_url, client })
    }

    /// Собирает URL из `base_url` ноды и переданных сегментов пути.
    ///
    /// Например, `["panel", "api", "clients", "get", "user@example.com"]`
    /// преобразуется в `{base_url}/panel/api/clients/get/user@example.com`.
    fn build_url(&self, segments: &[&str]) -> anyhow::Result<Url> {
        let mut url =
            Url::parse(&self.base_url).map_err(|e| anyhow!("Некорректный base_url ноды: {e}"))?;
        url.path_segments_mut()
            .map_err(|()| anyhow!("base_url не может быть базой для сегментов пути"))?
            .extend(segments);
        Ok(url)
    }

    /// Отправляет запрос к API панели и обрабатывает её ответ.
    ///
    /// Метод является общей точкой обработки всех запросов к 3x-ui:
    ///
    /// 1. Отправляет переданный [`RequestBuilder`].
    /// 2. Проверяет HTTP-статус ответа.
    /// 3. Читает тело HTTP-ответа целиком.
    /// 4. Десериализует тело в [`XuiResponse`].
    /// 5. Проверяет поле `success` в ответе API.
    ///
    /// Для 3x-ui v3.8.0+:
    ///
    /// - `401 Unauthorized` означает, что Bearer-токен не прошёл проверку;
    /// - `403 Forbidden` означает недостаточные права токена для запрошенного endpoint.
    ///
    /// Если `success` равен `true`, возвращает десериализованный ответ.
    /// Если `success` равен `false`, возвращает ошибку с сообщением из поля `msg`.
    ///
    /// При ошибке десериализации в сообщение включается HTTP-статус и первые 200 символов тела ответа. Если тело пустое, вместо него указывается `пустое тело ответа`.
    ///
    /// Возвращает ошибку отправки запроса, чтения тела, HTTP-ошибки аутентификации/авторизации, ошибки десериализации ответа или API-ошибки с `success: false`.
    async fn request(&self, builder: RequestBuilder) -> anyhow::Result<XuiResponse> {
        let response = builder.send().await?;
        let status = response.status();

        match status {
            StatusCode::UNAUTHORIZED => {
                return Err(anyhow!(
                    "3x-ui отклонил API-токен ноды (401 Unauthorized): \
                    токен невалиден, отключён или истёк"
                ));
            }
            StatusCode::FORBIDDEN => {
                return Err(anyhow!(
                    "3x-ui отклонил запрос ноды (403 Forbidden): \
                    API-токен не имеет необходимых прав доступа"
                ));
            }
            _ => {}
        }

        let raw_text = response.text().await?;

        let xui_response: XuiResponse = serde_json::from_str(&raw_text).map_err(|e| {
            let snippet = if raw_text.trim().is_empty() {
                "пустое тело ответа".to_string()
            } else {
                raw_text.chars().take(200).collect::<String>()
            };
            anyhow!("Не удалось разобрать ответ 3x-ui (статус {status}): {e}. Тело: {snippet}")
        })?;

        if xui_response.success {
            Ok(xui_response)
        } else {
            Err(anyhow!(
                "API-ошибка 3x-ui: {}",
                xui_response.msg.unwrap_or_else(|| "без текста".to_string())
            ))
        }
    }

    /// Создаёт или обновляет клиента в панели и синхронизирует его инбаунды.
    ///
    /// Метод ищет клиента по `email` через `GET /panel/api/clients/get/:email`.
    ///
    /// Если клиент не найден (`record not found`):
    /// - создаётся новый клиент через [`Self::add`];
    /// - при успешном создании метод завершается;
    /// - если `add` завершился ошибкой, выполняется одна повторная проверка наличия клиента;
    /// - если клиент всё же существует, выполняется сверка его состояния.
    ///
    /// Последний сценарий необходим для 3x-ui v3.8.0+, где операции с несколькими `inboundIds` применяются независимо:
    /// `success: false` не гарантирует полного отката уже выполненных изменений.
    ///
    /// Если клиент найден:
    /// - обновляется его конфиг через [`Self::update`];
    /// - сравниваются существующие `inboundIds` с `required_inbounds`;
    /// - клиент привязывается только к отсутствующим инбаундам через [`Self::attach`].
    ///
    /// Существующие привязки не удаляются.
    ///
    /// Пустой `inboundIds` считается допустимым состоянием существующего клиента.
    /// В этом случае сверка просто обновляет клиента и привязывает все `required_inbounds`.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если:
    /// - не удалось выполнить запрос или разобрать ответ API;
    /// - API вернул неожидаемый формат `obj`;
    /// - у найденного клиента отсутствует `inboundIds`, либо оно не является массивом;
    /// - `inboundIds` содержит невалидный ID;
    /// - `add` завершился ошибкой, клиент при повторной проверке не найден, а исходную ошибку нельзя устранить автоматически;
    /// - не удалось обновить клиента или привязать его к инбаундам.
    pub async fn upsert(
        &self,
        required_inbounds: Vec<i32>,
        email: &str,
        config: &ClientObj,
    ) -> anyhow::Result<()> {
        match self.fetch_existing_inbounds(email).await? {
            Some(existing_inbounds) => {
                self.reconcile(email, config, &required_inbounds, &existing_inbounds).await
            }
            None => match self.add(required_inbounds.clone(), config).await {
                Ok(()) => {
                    info!(email, ?required_inbounds, "Новый клиент создан");
                    Ok(())
                }
                Err(add_err) => match self.fetch_existing_inbounds(email).await {
                    Ok(Some(existing_inbounds)) => {
                        info!(email, error = %add_err, "add() завершился ошибкой, но клиент обнаружен - выполняю сверку");
                        self.reconcile(email, config, &required_inbounds, &existing_inbounds).await
                    }
                    Ok(None) => Err(add_err),
                    Err(probe_err) => Err(anyhow!(
                        "Не удалось создать клиента в 3x-ui: {add_err}; \
                        повторная проверка существования клиента также завершилась ошибкой: {probe_err}"
                    )),
                },
            },
        }
    }

    /// Возвращает список `inboundIds`, к которым уже привязан клиент с данным `email`,
    /// либо `None`, если клиент не найден (`record not found`).
    ///
    /// Для найденного клиента ответ API должен содержать в поле `obj` JSON-объект с массивом `inboundIds`.
    /// Пустой массив допускается: он означает существующего клиента без текущих привязок к инбаундам.
    ///
    /// Каждый элемент массива должен содержать целочисленный ID как [`i32`].
    async fn fetch_existing_inbounds(&self, email: &str) -> anyhow::Result<Option<Vec<i32>>> {
        let get_url = self.build_url(&["panel", "api", "clients", "get", email])?;

        match self.request(self.client.get(get_url)).await {
            Ok(response) => match response.obj {
                Some(Value::Object(map)) => {
                    let inbounds_array =
                        map.get("inboundIds").and_then(Value::as_array).ok_or_else(|| {
                            anyhow!("API вернул клиента, но 'inboundIds' отсутствует или не массив")
                        })?;

                    let ids: Vec<i32> = inbounds_array
                        .iter()
                        .map(|value| {
                            value.as_i64().and_then(|id| i32::try_from(id).ok()).ok_or_else(|| {
                                anyhow!("Массив 'inboundIds' содержит невалидный ID")
                            })
                        })
                        .collect::<anyhow::Result<_>>()?;

                    Ok(Some(ids))
                }
                _ => Err(anyhow!(
                    "Неожиданный формат 'obj' в ответе API: ожидался JSON-объект клиента"
                )),
            },
            Err(error) if error.to_string().contains(ERR_RECORD_NOT_FOUND) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Обновляет метаданные клиента и привязывает его к отсутствующим из `required` инбаундам.
    ///
    /// Используется как для обычного сценария существующего клиента,
    /// так и для сверки после частично успешного [`Self::add`].
    async fn reconcile(
        &self,
        email: &str,
        config: &ClientObj,
        required: &[i32],
        existing: &[i32],
    ) -> anyhow::Result<()> {
        self.update(email, config).await?;
        info!(email, "Метаданные клиента обновлены");

        let missing_inbounds: Vec<i32> =
            required.iter().copied().filter(|id| !existing.contains(id)).collect();

        if !missing_inbounds.is_empty() {
            self.attach(email, missing_inbounds.clone()).await?;

            info!(
                email,
                ?missing_inbounds,
                "Клиент привязан к новым инбаундам"
            );
        }

        Ok(())
    }

    /// Создаёт клиента и привязывает его к указанным инбаундам.
    ///
    /// Вызывает `POST /panel/api/clients/add`.
    ///
    /// В тело запроса передаются параметры клиента и список ID инбаундов:
    ///
    /// ```json
    /// {
    ///   "client": { ... },
    ///   "inboundIds": [1, 2, 3]
    /// }
    /// ```
    async fn add(&self, inbound_ids: Vec<i32>, config: &ClientObj) -> anyhow::Result<()> {
        let url = self.build_url(&["panel", "api", "clients", "add"])?;
        let payload = json!({
            "client": config,
            "inboundIds": inbound_ids,
        });
        self.request(self.client.post(url).json(&payload)).await?;
        Ok(())
    }

    /// Обновляет метаданные существующего клиента.
    ///
    /// Вызывает `POST /panel/api/clients/update/:email`.
    ///
    /// В отличие от [`Self::add`] запрос передаётся в виде плоского JSON конфига клиента без дополнительной обертки `client`.
    async fn update(&self, email: &str, config: &ClientObj) -> anyhow::Result<()> {
        let url = self.build_url(&["panel", "api", "clients", "update", email])?;
        self.request(self.client.post(url).json(config)).await?;
        Ok(())
    }

    /// Привязывает существующего клиента к указанным инбаундам.
    ///
    /// Вызывает `POST /panel/api/clients/:email/attach`.
    ///
    /// В тело запроса передаётся список ID инбаундов:
    ///
    /// ```json
    /// {
    ///   "inboundIds": [1, 2, 3]
    /// }
    /// ```
    async fn attach(&self, email: &str, inbound_ids: Vec<i32>) -> anyhow::Result<()> {
        let url = self.build_url(&["panel", "api", "clients", email, "attach"])?;
        let payload = json!({
            "inboundIds": inbound_ids
        });
        self.request(self.client.post(url).json(&payload)).await?;
        Ok(())
    }

    /// Отключает всех клиентов, привязанных к указанному Telegram ID.
    ///
    /// Выполняет `GET /panel/api/clients/get/tgId/:tg_id` и для каждого найденного клиента устанавливает `enable` в `false`.
    ///
    /// Перед отправкой клиента обратно в панель метод исправляет два несовместимых формата, которые 3x-ui может возвращать из API:
    ///
    /// - `id` может возвращаться как числовой идентификатор, хотя endpoint обновления ожидает UUID в виде строки. Поэтому `id` заменяется значением из поля `uuid`;
    /// - `allowedIPs` может возвращаться как пустая строка `""`, тогда как endpoint обновления ожидает массив строк. В этом случае значение заменяется на пустой массив.
    ///
    /// Обновлённый клиент передаётся в`POST /panel/api/clients/update/:email` в виде плоского JSON-объекта.
    ///
    /// Если API не вернул массив клиентов или массив оказался пустым, метод ничего не делает и возвращает `Ok(())`.
    ///
    /// Элементы ответа без корректного объекта `client` или без поля `email` пропускаются без ошибки.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если:
    /// - не удалось выполнить запрос или разобрать ответ API;
    /// - не удалось сформировать URL;
    /// - не удалось обновить одного из найденных клиентов.
    pub async fn disable_by_tgid(&self, tg_id: i64) -> anyhow::Result<()> {
        let url =
            self.build_url(&["panel", "api", "clients", "get", "tgId", &tg_id.to_string()])?;
        let response = self.request(self.client.get(url)).await?;

        let clients = match response.obj {
            Some(Value::Array(arr)) if !arr.is_empty() => arr,
            _ => return Ok(()),
        };

        for mut wrapper in clients {
            if let Some(client_obj) = wrapper.get_mut("client").and_then(|c| c.as_object_mut()) {
                let email = match client_obj.get("email").and_then(|e| e.as_str()) {
                    Some(e) => e.to_string(),
                    None => continue,
                };

                if let Some(uuid_val) = client_obj.get("uuid").cloned() {
                    client_obj.insert("id".to_string(), uuid_val);
                }

                if let Some(Value::String(s)) = client_obj.get("allowedIPs")
                    && s.is_empty()
                {
                    client_obj.insert("allowedIPs".to_string(), Value::Array(vec![]));
                }

                client_obj.insert("enable".to_string(), Value::Bool(false));

                let update_url = self.build_url(&["panel", "api", "clients", "update", &email])?;
                self.request(
                    self.client.post(update_url).json(client_obj).timeout(Duration::from_secs(30)),
                )
                .await?;

                info!(
                    email,
                    tg_id, "Клиент в 3x-ui успешно отключен (enable: false)"
                );
            }
        }

        Ok(())
    }
}
