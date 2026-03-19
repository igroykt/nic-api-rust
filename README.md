# nic-api-rust

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)

Библиотека на Rust для управления DNS-записями через [DNS API NIC.RU](https://nic.ru).

---

## Возможности

- Аутентификация OAuth2 и управление токенами (password grant + refresh)
- **Автоматическое обновление токена и повтор запроса** при истечении (ошибка 4097)
- Просмотр DNS-сервисов и зон
- **Список всех зон** по всем сервисам (`zones_all()`)
- Чтение, добавление и удаление DNS-записей
- Поддержка 13 типов DNS-записей (A, AAAA, CNAME, MX, NS, TXT, SOA, SRV, PTR, DNAME, HINFO, NAPTR, RP)
- **Жизненный цикл зон** — создание, удаление, перенос (`create_zone`, `delete_zone`, `move_zone`)
- **Импорт и экспорт зон** в формате BIND (`zone_export`, `zone_import`)
- **Откат незафиксированных изменений** (`rollback`)
- **Управление TTL по умолчанию** (`get_default_ttl`, `set_default_ttl`)
- **История ревизий зоны** (`zone_revisions`)
- **Управление AXFR / zone transfer** — список IP для трансфера (`get_axfr_ips`, `set_axfr_ips`)
- **Управление master-серверами** для вторичных зон (`get_masters`, `set_masters`)
- Асинхронный API на основе [Tokio](https://tokio.rs)

---

## Установка

Добавьте в `Cargo.toml`:

```toml
[dependencies]
nic-api-rust = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

---

## Быстрый старт

```rust
use nic_api_rust::{DnsApi, DnsRecord, ARecord};

#[tokio::main]
async fn main() -> nic_api_rust::Result<()> {
    // 1. Инициализация клиента
    let api = DnsApi::new(
        "your-app-login",             // логин OAuth2-приложения
        "your-app-password",          // пароль OAuth2-приложения
        None,                         // существующий Token, если есть
        Some(3600),                   // TTL офлайн-режима в секундах (для refresh-токена), или None
        Some("/.+/".to_string()),     // область OAuth2, или None для значения по умолчанию
        Some("your-service".to_string()), // NIC_SERVICE_ID: сервис по умолчанию (обязателен для DNS-операций)
        Some("example.com".to_string()),  // NIC_ZONE: имя зоны по умолчанию
    );

    // 2. Аутентификация с учётными данными аккаунта NIC.RU
    // get_token() и refresh_token() принимают &self, поэтому mut не нужен.
    let token = api.get_token("nic-username", "nic-password").await?;
    println!("Токен доступа: {}", token.access_token);

    // 3. Список DNS-сервисов
    let services = api.services().await?;
    for svc in &services {
        println!("Сервис: {}", svc.name);
    }

    // 4. Список зон (фильтр по сервису — или None для service_id)
    let zones = api.zones(None).await?;
    for zone in &zones {
        println!("Зона: {}", zone.name);
    }

    // 4b. Список всех зон по всем сервисам
    let all_zones = api.zones_all().await?;
    println!("Всего зон: {}", all_zones.len());

    // 5. Список существующих записей
    let records = api.records(None, None).await?;
    for record in &records {
        println!("Запись: {} (id={:?})", record.name(), record.id());
    }

    // 6. Добавление новой A-записи
    let new_record = DnsRecord::A(ARecord::new("test", "1.2.3.4").with_ttl(300)?);
    let _added = api.add_record(vec![new_record], None, None).await?;

    // 7. Применение изменений в зоне
    api.commit(None, None).await?;
    println!("Изменения применены.");

    // 8. Удаление записи по ID
    let record_id = records
        .iter()
        .find_map(|r| r.id())
        .expect("записи не найдены");
    api.delete_record(record_id, None, None).await?;
    api.commit(None, None).await?;
    println!("Запись {} удалена и изменения применены.", record_id);

    Ok(())
}
```

---

## Переменные окружения

Пример в `examples/basic_usage.rs` использует следующие переменные окружения:

| Переменная | Обязательная | Описание |
|------------|--------------|----------|
| `NIC_APP_LOGIN` | ✅ | Логин OAuth2-приложения из консоли разработчика NIC.RU |
| `NIC_APP_PASSWORD` | ✅ | Пароль OAuth2-приложения |
| `NIC_USERNAME` | ✅ | Имя пользователя аккаунта NIC.RU |
| `NIC_PASSWORD` | ✅ | Пароль аккаунта NIC.RU |
| `NIC_SERVICE_ID` | ✅ | Идентификатор DNS-сервиса |
| `NIC_ZONE` | ✅ | Имя DNS-зоны |

---

## Автоматическое обновление токена

Все API-методы автоматически обнаруживают ошибку истечения токена (код 4097), обновляют токен через хранящийся refresh-токен и прозрачно повторяют исходный запрос. Ручное перехватывание `DnsApiError::ExpiredToken` больше не требуется для большинства случаев.

Вы также можете управлять токеном вручную:

```rust
// Получить текущий токен
let saved_token = api.token().await;

// Восстановить сохранённый токен
if let Some(t) = saved_token {
    api.set_token(t).await;
}

// Обновить вручную
let refreshed = api.refresh_token(&token.refresh_token).await?;
```

---

## Жизненный цикл зон

```rust
// Создание новой зоны
let zone = api.create_zone("newzone.example.com", Some("MY_SERVICE")).await?;
println!("Создана зона: {}", zone.name);

// Перенос зоны в другой сервис
api.move_zone(Some("example.com"), "TARGET_SERVICE", Some("SOURCE_SERVICE")).await?;

// Удаление зоны
api.delete_zone("newzone.example.com", Some("MY_SERVICE")).await?;
```

---

## Импорт и экспорт зоны (формат BIND)

```rust
// Экспорт зоны в формате BIND
let bind_text = api.zone_export(Some("MY_SERVICE"), Some("example.com")).await?;
println!("{}", bind_text);

// Импорт зоны из формата BIND (заменяет существующие записи)
let zone_data = "$ORIGIN example.com.\n@ 3600 IN SOA ...\n";
api.zone_import(zone_data, Some("MY_SERVICE"), Some("example.com")).await?;
```

---

## Откат незафиксированных изменений

```rust
// Отменить все несохранённые (staged) изменения в зоне
api.rollback(Some("MY_SERVICE"), Some("example.com")).await?;
```

---

## Управление TTL по умолчанию

```rust
// Получить текущий TTL по умолчанию (из SOA-записи)
let ttl = api.get_default_ttl(Some("MY_SERVICE"), Some("example.com")).await?;
println!("TTL: {} секунд", ttl);

// Установить новый TTL
api.set_default_ttl(1800, Some("MY_SERVICE"), Some("example.com")).await?;
```

---

## История ревизий зоны

```rust
let revisions = api.zone_revisions(Some("MY_SERVICE"), Some("example.com")).await?;
for rev in &revisions {
    println!("Ревизия #{} — {} — IP: {}", rev.number, rev.date, rev.ip);
}
```

---

## Управление AXFR (zone transfer)

```rust
// Получить список IP, которым разрешён трансфер зоны
let ips = api.get_axfr_ips(Some("MY_SERVICE"), Some("example.com")).await?;
println!("Разрешённые IP: {:?}", ips);

// Установить новый список IP
api.set_axfr_ips(&["192.0.2.1", "198.51.100.2"], Some("MY_SERVICE"), Some("example.com")).await?;
```

---

## Управление master-серверами (вторичные зоны)

```rust
// Получить список master-серверов
let masters = api.get_masters(Some("MY_SERVICE"), Some("example.com")).await?;
println!("Master-серверы: {:?}", masters);

// Установить новый список master-серверов
api.set_masters(&["192.0.2.10", "198.51.100.20"], Some("MY_SERVICE"), Some("example.com")).await?;
```

---

## Справочник API

### `DnsApi`

```rust
pub struct DnsApi {
    pub service_id: Option<String>,
    pub zone: Option<String>,
    // ...
}
```

#### Конструктор

```rust
DnsApi::new(
    app_login: impl Into<String>,
    app_password: impl Into<String>,
    token: Option<Token>,
    offline: Option<u64>,
    scope: Option<String>,
    service_id: Option<String>,
    zone: Option<String>,
) -> Self
```

| Параметр | Описание |
|----------|----------|
| `app_login` | Логин OAuth2-приложения из консоли разработчика NIC.RU |
| `app_password` | Пароль OAuth2-приложения |
| `token` | Существующий `Token` для возобновления сессии |
| `offline` | TTL refresh-токена в секундах (`Some(...)`) или `None` для отключения офлайн-доступа |
| `scope` | Строка области OAuth2 (например, `"/.+/"`) или `None` для значения по умолчанию |
| `service_id` | Идентификатор DNS-сервиса |
| `zone` | Имя DNS-зоны |

#### Методы

| Метод | Описание |
|-------|----------|
| `get_token(username, password) -> Result<Token>` | Получение токена доступа по учётным данным аккаунта NIC.RU |
| `refresh_token(refresh_token) -> Result<Token>` | Обновление существующего токена доступа |
| `token() -> Option<Token>` | Получение текущего токена (если установлен) |
| `set_token(token)` | Установка токена напрямую |
| `services() -> Result<Vec<NicService>>` | Список всех DNS-сервисов, доступных аккаунту |
| `zones(service) -> Result<Vec<NicZone>>` | Список зон сервиса (None = service_id) |
| `zones_all() -> Result<Vec<NicZone>>` | Список всех зон по всем сервисам (`GET /dns-master/zones`) |
| `create_zone(zone_name, service) -> Result<NicZone>` | Создание новой DNS-зоны |
| `delete_zone(zone_name, service) -> Result<()>` | Удаление DNS-зоны |
| `move_zone(zone, target_service, service) -> Result<()>` | Перенос зоны в другой сервис |
| `zone_export(service, zone) -> Result<String>` | Экспорт зоны в формате BIND |
| `zone_import(content, service, zone) -> Result<()>` | Импорт зоны из формата BIND (text/plain) |
| `rollback(service, zone) -> Result<()>` | Откат незафиксированных изменений зоны |
| `get_default_ttl(service, zone) -> Result<u32>` | Получение TTL по умолчанию (из SOA) |
| `set_default_ttl(ttl, service, zone) -> Result<()>` | Установка TTL по умолчанию |
| `zone_revisions(service, zone) -> Result<Vec<NicZoneRevision>>` | История ревизий зоны |
| `get_axfr_ips(service, zone) -> Result<Vec<String>>` | Список IP для zone transfer (AXFR) |
| `set_axfr_ips(ips, service, zone) -> Result<()>` | Установка IP для zone transfer (AXFR) |
| `get_masters(service, zone) -> Result<Vec<String>>` | Список master-серверов (вторичная зона) |
| `set_masters(ips, service, zone) -> Result<()>` | Установка master-серверов (вторичная зона) |
| `records(service, zone) -> Result<Vec<DnsRecord>>` | Список DNS-записей в зоне |
| `add_record(records, service, zone) -> Result<Vec<DnsRecord>>` | Добавление одной или нескольких DNS-записей |
| `delete_record(record_id, service, zone) -> Result<()>` | Удаление DNS-записи по её числовому ID |
| `commit(service, zone) -> Result<()>` | Применение ожидающих изменений в зоне |
| `set_service_id(service: impl Into<String>)` | Установка DNS-сервиса по умолчанию (`&mut self`) |
| `set_zone(zone: impl Into<String>)` | Установка DNS-зоны по умолчанию (`&mut self`) |

Для методов `zones()`, `records()`, `add_record()`, `delete_record()`, `commit()` и всех новых методов управления зонами передача `None` для `service` или `zone` приведёт к использованию `service_id` / `zone` соответственно.

> **Важно:** `service_id` (также читается из переменной окружения `NIC_SERVICE_ID` в примерах) **обязателен** для всех DNS-операций — `records()`, `add_record()`, `delete_record()`, `commit()` и `zones()`. Без него имя сервиса необходимо передавать явно при каждом вызове. `zone` (читается из `NIC_ZONE`) аналогично обязателен для операций на уровне записей.

---

### `Token` и `TokenManager`

`Token` хранит учётные данные OAuth2:

```rust
pub struct Token {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
    pub scope: String,
}
```

`TokenManager` управляет жизненным циклом токена:

| Метод | Описание |
|-------|----------|
| `TokenManager::new()` | Создание нового пустого менеджера токенов |
| `set_token(token)` | Сохранение токена |
| `get_token() -> Option<&Token>` | Получение сохранённого токена |
| `access_token() -> Option<&str>` | Получение только строки токена доступа |
| `get_token_with_password(...)` | Получение токена через OAuth2 password grant |
| `refresh_token(...)` | Обновление текущего токена |

---

## Поддерживаемые типы записей

| Тип | Структура | Конструктор | Есть TTL | Чтение из API | Запись в API |
|-----|-----------|-------------|----------|---------------|--------------|
| A | `ARecord` | `ARecord::new(name, a)` | ✅ | ✅ | ✅ |
| AAAA | `AaaaRecord` | `AaaaRecord::new(name, aaaa)` | ✅ | ✅ | ✅ |
| CNAME | `CnameRecord` | `CnameRecord::new(name, cname)` | ✅ | ✅ | ✅ |
| MX | `MxRecord` | `MxRecord::new(name, preference, exchange)` | ✅ | ✅ | ✅ |
| NS | `NsRecord` | `NsRecord::new(name, ns)` | ✅ | ✅ | ✅ |
| TXT | `TxtRecord` | `TxtRecord::new(name, txt)` | ✅ | ✅ | ✅ |
| SOA | `SoaRecord` | `SoaRecord::new(name, mname, rname, serial, refresh, retry, expire, minimum)` | ✅ | ✅ | ✅ |
| SRV | `SrvRecord` | `SrvRecord::new(name, priority, weight, port, target)` | ✅ | ✅ | ✅ |
| PTR | `PtrRecord` | `PtrRecord::new(name, ptr)` | ✅ | ✅ | ✅ |
| DNAME | `DnameRecord` | `DnameRecord::new(name, dname)` | ✅ | ✅ | ✅ |
| HINFO | `HinfoRecord` | `HinfoRecord::new(name, hardware, os)` | ✅ | ✅ | ✅ |
| NAPTR | `NaptrRecord` | `NaptrRecord::new(name, order, preference, flags, service)` | ✅ | ✅ | ✅ |
| RP | `RpRecord` | `RpRecord::new(name, mbox, txt)` | ✅ | ✅ | ✅ |

Все типы записей с поддержкой TTL имеют метод-строитель:

```rust
let record = ARecord::new("www", "1.2.3.4").with_ttl(3600)?;
```

### Методы `DnsRecord`

```rust
record.id() -> Option<u64>   // числовой ID записи, присутствует только после получения из API
record.name() -> &str        // имя / метка записи
record.to_xml() -> Result<String>  // сериализация в формат XML для NIC.RU
```

---

## Обработка ошибок

Все методы, которые могут завершиться ошибкой, возвращают `nic_api_rust::Result<T>`, то есть `Result<T, DnsApiError>`.

```rust
use nic_api_rust::DnsApiError;

match api.records(None, None).await {
    Ok(records) => { /* ... */ }
    Err(DnsApiError::ExpiredToken) => { /* токен истёк — обычно обрабатывается автоматически */ }
    Err(DnsApiError::ZoneNotFound(zone)) => eprintln!("Зона не найдена: {zone}"),
    Err(e) => eprintln!("Ошибка: {e}"),
}
```

### Варианты `DnsApiError`

| Вариант | Описание |
|---------|----------|
| `ExpiredToken` | Токен доступа истёк (автоматически обрабатывается при обычных запросах) |
| `InvalidRecord(String)` | Запись не прошла валидацию |
| `ServiceNotFound(String)` | Указанный DNS-сервис не существует |
| `ZoneNotFound(String)` | Указанная DNS-зона не существует |
| `ZoneAlreadyExists(String)` | Попытка создать уже существующую зону |
| `InvalidDomainName(String)` | Доменное имя имеет неверный формат |
| `HttpError(reqwest::Error)` | Ошибка транспортного уровня HTTP |
| `XmlError(String)` | Ошибка разбора или сериализации XML |
| `OAuth2Error(String)` | Ошибка аутентификации OAuth2 |
| `ApiError(String)` | Общая ошибка API NIC.RU |
| `InvalidTtl` | Значение TTL выходит за допустимые пределы |
| `InvalidRecordId` | ID записи недействителен или отсутствует |

---

## Лицензия

Проект распространяется под лицензией [GNU General Public License v3.0](LICENSE).
