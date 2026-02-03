# nic-api-rust

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

Rust библиотека для управления DNS зонами и записями через API NIC.RU (Ru-Center).

Это порт популярной Python библиотеки [nic-api](https://github.com/andr1an/nic-api) на язык Rust с полной поддержкой асинхронных операций.

## 🚀 Возможности

- ✅ OAuth2 аутентификация (Resource Owner Password Credentials Grant + Refresh Token)
- ✅ Управление сервисами и DNS зонами
- ✅ CRUD операции для DNS записей
- ✅ Поддержка 13 типов DNS записей: SOA, NS, A, AAAA, CNAME, MX, TXT, SRV, PTR, DNAME, HINFO, NAPTR, RP
- ✅ Commit-based workflow для применения изменений
- ✅ Асинхронный API на базе tokio и reqwest
- ✅ Полная обработка ошибок

## 📦 Установка

Добавьте в ваш `Cargo.toml`:

```toml
[dependencies]
nic-api-rust = "0.1"
tokio = { version = "1", features = ["full"] }
```

## 🔧 Начало работы

### Получение OAuth credentials

Для работы с API необходимо зарегистрировать OAuth приложение на странице:
https://www.nic.ru/manager/oauth.cgi?step=oauth.app_register

Вы получите:
- `app_login` - логин OAuth приложения
- `app_password` - пароль OAuth приложения

### Базовый пример

```rust
use nic_api_rust::{DnsApi, models::ARecord, DnsRecord};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация клиента
    let mut api = DnsApi::new(
        "your_app_login",
        "your_app_password",
        None,  // существующий токен (опционально)
        None,  // время жизни токена (по умолчанию 3600 сек)
        None,  // scope (по умолчанию ".+:/dns-master/.+")
    );

    // Аутентификация
    api.get_token("Your_account/NIC-D", "Your_password").await?;

    // Получение списка сервисов
    let services = api.services().await?;
    println!("Доступные сервисы: {:?}", services);

    // Установка сервиса и зоны по умолчанию
    api.default_service = Some("MY_SERVICE".to_string());
    api.default_zone = Some("example.com".to_string());

    // Получение DNS записей
    let records = api.records(None, None).await?;

    // Создание A-записи
    let record = ARecord::new("www", "192.168.1.1")
        .with_ttl(3600)?;
    
    api.add_record(vec![DnsRecord::A(record)], None, None).await?;

    // Применение изменений
    api.commit(None, None).await?;

    // Удаление записи по ID
    api.delete_record(12345, None, None).await?;
    api.commit(None, None).await?;

    Ok(())
}
```

## 📚 Основные методы API

### Аутентификация

```rust
// Получить токен
let token = api.get_token(username, password).await?;

// Обновить токен
let new_token = api.refresh_token(refresh_token).await?;
```

### Сервисы и зоны

```rust
// Получить список сервисов
let services = api.services().await?;

// Получить зоны в сервисе
let zones = api.zones(Some("MY_SERVICE")).await?;

// Применить изменения
api.commit(Some("MY_SERVICE"), Some("example.com")).await?;
```

### DNS записи

```rust
// Получить все записи
let records = api.records(Some("MY_SERVICE"), Some("example.com")).await?;

// Добавить записи
let a_record = ARecord::new("subdomain", "1.2.3.4").with_ttl(3600)?;
api.add_record(vec![DnsRecord::A(a_record)], None, None).await?;

// Удалить запись по ID
api.delete_record(record_id, None, None).await?;
```

### Использование значений по умолчанию

Для упрощения вызовов можно установить сервис и зону по умолчанию:

```rust
api.default_service = Some("MY_SERVICE".to_string());
api.default_zone = Some("example.com".to_string());

// Теперь можно вызывать методы без указания service и zone
api.records(None, None).await?;
api.commit(None, None).await?;
```

## 🧪 Запуск примеров

```bash
# Установите переменные окружения
export NIC_APP_LOGIN="your_app_login"
export NIC_APP_PASSWORD="your_app_password"
export NIC_USERNAME="your_username"
export NIC_PASSWORD="your_password"

# Запустите пример
cargo run --example basic_usage
```

## 🛠️ Разработка

```bash
# Компиляция
cargo build

# Запуск тестов
cargo test

# Проверка кода
cargo clippy -- -D warnings

# Генерация документации
cargo doc --open
```

## 📝 Лицензия

Этот проект распространяется под лицензией GNU General Public License v3.0 - см. файл [LICENSE](LICENSE) для деталей.

Это соответствует лицензии оригинальной Python библиотеки [nic-api](https://github.com/andr1an/nic-api).

## 🤝 Вклад в проект

Приветствуются любые вклады в развитие проекта! Пожалуйста:

1. Форкните репозиторий
2. Создайте ветку для ваших изменений (`git checkout -b feature/amazing-feature`)
3. Закоммитьте изменения (`git commit -m 'Add some amazing feature'`)
4. Запушьте в ветку (`git push origin feature/amazing-feature`)
5. Откройте Pull Request

## 🔗 Ссылки

- [Оригинальная Python библиотека nic-api](https://github.com/andr1an/nic-api)
- [Документация NIC.RU API](https://www.nic.ru/help/api-dns-hostinga_3643.html)
- [Регистрация OAuth приложения](https://www.nic.ru/manager/oauth.cgi?step=oauth.app_register)

## ⚠️ Важно

**Всегда проверяйте наличие незакоммиченных изменений в зоне перед внесением модификаций!** 
Ваш коммит применит все несохранённые изменения в зоне.
