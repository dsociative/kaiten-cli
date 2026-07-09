# kaiten — CLI и MCP-сервер для трекера Kaiten

Дата: 2026-07-09
Статус: утверждён

## Цель

Консольная утилита в стиле `gh`/`glab` и MCP-сервер для корпоративного трекера Kaiten
(`https://{company}.kaiten.ru`), чтобы работать с трекером из терминала и через агента,
не заходя в веб-интерфейс.

## Контекст API

- Официальной OpenAPI-спецификации у Kaiten нет; документация — https://developers.kaiten.ru/.
- Аутентификация: `Authorization: Bearer <token>` (токен из профиля пользователя).
- Базовый URL: `https://{company}.kaiten.ru/api/latest`.
- Rate limit: 50 req/s; при превышении — HTTP 429 с заголовками `X-RateLimit-Remaining`,
  `X-RateLimit-Reset`.
- Решение: типы клиента пишутся вручную по документации. Community-спека
  (AllDmeat/kaiten-sdk) не используется — ни для кодогенерации, ни в тестах.

## Скоуп v1

1. Мои карточки и поиск: список, фильтрация по доске/пространству/колонке/тегу/типу,
   текстовый поиск, просмотр карточки с комментариями.
2. Создание и правка карточек: создать, изменить заголовок/описание/тип/ASAP,
   участники, перемещение по колонкам/дорожкам/доскам.
3. Комментарии и чеклисты: чтение/добавление комментариев, чеклисты и их пункты
   (добавить, отметить/снять).
4. Навигация по структуре: пространства, доски (с колонками и дорожками), теги,
   типы карточек, пользователи.

### Вне скоупа v1

Файлы/вложения, тайм-логи, спринты/итерации, автоматизации, документы,
редактирование custom properties (в просмотре карточки отображаются как есть),
CI и релизные бинарники (инструмент пока личный, ставится `cargo install --path`).

## Архитектура

Cargo workspace, два крейта:

```
kaiten-cli/
├─ Cargo.toml            # workspace
├─ crates/
│  ├─ kaiten-client/     # либа: типизированный API-клиент
│  │  ├─ src/
│  │  │  ├─ lib.rs
│  │  │  ├─ models/      # Card, Board, Space, Column, Lane, User, Comment,
│  │  │  │               # Checklist, ChecklistItem, Tag, CardType
│  │  │  ├─ api/         # методы, сгруппированные по ресурсам
│  │  │  └─ error.rs
│  │  └─ tests/          # wiremock-тесты
│  └─ kaiten/            # бинарник `kaiten`
│     ├─ src/
│     │  ├─ main.rs
│     │  ├─ commands/    # card, board, space, tag, auth, api, completion
│     │  ├─ mcp/         # kaiten mcp serve (rmcp, stdio)
│     │  ├─ output/      # таблицы (comfy-table) + --json
│     │  ├─ config.rs    # ~/.config/kaiten/config.toml + env
│     │  └─ error.rs     # CliError (thiserror)
│     └─ tests/          # assert_cmd + wiremock + insta
```

MCP-сервер — сабкоманда `kaiten mcp serve` в том же бинарнике: одна установка,
общий конфиг и авторизация с CLI.

## Стек

| Зона | Выбор |
|---|---|
| Async/HTTP | `tokio`, `reqwest` (rustls, без openssl) |
| Сериализация | `serde`, `serde_json`, `serde_path_to_error` |
| CLI | `clap` v4 (derive), `clap_complete` |
| MCP | `rmcp` (официальный Rust MCP SDK), транспорт stdio, схемы через `schemars` |
| Ошибки | `thiserror` во всех крейтах; `anyhow` запрещён |
| Логи | `tracing`, `tracing-subscriber` (вывод в stderr) |
| Таблицы | `comfy-table` |
| Конфиг | `toml`, каталог по `dirs` |
| Тесты | `wiremock`, `assert_cmd`, `insta` |

## kaiten-client

- Конструктор: `KaitenClient::new(base_url, token)`.
- API по ресурсам: `client.cards().list(&CardFilter)`, `.get(id)`, `.create(&CreateCard)`,
  `.update(id, &UpdateCard)`; `client.comments(card_id)`, `client.checklists(card_id)`,
  `client.boards()`, `client.spaces()`, `client.tags()`, `client.card_types()`,
  `client.users()` (включая `current()`).
- Модели — рукописные serde-структуры по документации developers.kaiten.ru.
  Десериализация толерантная: неизвестные поля игнорируются, необязательные — `Option`.
  Custom properties карточки — `serde_json::Value` (read-only).
- Ошибки (`thiserror`):
  - `Api { status, message, body }` — тело ошибки API доносится до пользователя;
  - `RateLimited { retry_after }`;
  - `Network(reqwest::Error)`;
  - `Decode { path, source }` — через `serde_path_to_error`: указывает точное поле,
    на котором разошлись типы с реальным API.
- Автоповтор на 429 по `X-RateLimit-Reset`, ограниченное число попыток, затем ошибка.
- Каждый запрос — событие `tracing`: метод, путь, статус, длительность (debug);
  тела запросов/ответов (trace); заголовок Authorization всегда редактируется.

## CLI

```
kaiten auth login|status              # login: домен+токен → проверка через /users/current
kaiten space list
kaiten board list --space <id>
kaiten board view <id>                # колонки и дорожки с ID (нужны для move)
kaiten card list [--board] [--space] [--column] [--mine] [--member] [--query]
                 [--tag] [--type] [--limit]
kaiten card view <id|url> [--comments]
kaiten card create --board <id> --title <t> [--column] [--lane] [--description] [--type]
kaiten card edit <id> [--title] [--description] [--type] [--asap]
kaiten card move <id> --column <id> [--lane] [--board]
kaiten card member add|remove <id> <user-id|email>
kaiten card comment add <id> --body <text>
kaiten card comment list <id>
kaiten card checklist list <id>
kaiten card checklist add <id> --name <n>
kaiten card checklist item add <card> <checklist> --text <t>
kaiten card checklist item check|uncheck <card> <checklist> <item>
kaiten card tag add|remove <id> <tag>
kaiten card-type list
kaiten tag list
kaiten api <METHOD> <path> [--data <json>]   # сырой доступ, как gh api
kaiten completion <shell>                    # zsh/bash/fish
kaiten mcp serve
```

- `card view` принимает числовой ID или полный URL карточки из браузера
  (ID извлекается из URL).
- Вывод: человеку — таблицы; флаг `--json` у каждой команды печатает полный JSON
  ответа (для скриптов и агентов).
- Интерфейс и help-тексты — на английском.
- Ошибки CLI — enum `CliError` (`thiserror`), оборачивает `KaitenError`, ошибки
  конфига и IO через `#[from]`. Ненулевые exit-коды при ошибках.

### Конфигурация

- `~/.config/kaiten/config.toml`: `domain`, `token`; файл создаётся с правами 600.

  ```toml
  domain = "mycompany"    # → https://mycompany.kaiten.ru/api/latest
  token = "..."

  [defaults]              # необязательно
  space = 123             # используется card list и т.п., если флаг не задан
  board = 456             # используется card list/create, если флаг не задан
  ```

- Приоритет источников: флаги CLI → env → config.toml.
- Env-переопределения: `KAITEN_TOKEN`, `KAITEN_DOMAIN`, `KAITEN_BASE_URL`
  (последняя — полный базовый URL, перекрывает domain; используется в тестах
  для подмены на wiremock).
- `kaiten auth login` спрашивает домен и токен (скрытый ввод), проверяет их
  запросом `/users/current` и только после успеха сохраняет конфиг;
  `kaiten auth status` показывает домен, текущего пользователя и источник
  токена (env или файл).
- Резолв токена — слоёный (env → файл), чтобы позже можно было добавить
  источник `keyring` (macOS Keychain) одной веткой. В v1 кейчейн сознательно
  не поддерживается: пересборка бинарника вызывает повторные промпты доступа,
  а фоновый MCP-сервер может зависнуть на GUI-промпте.

## MCP-сервер

- `kaiten mcp serve`, транспорт stdio; регистрация в агенте одной командой.
- Инструменты (16), зеркалят CLI: `list_spaces`, `list_boards`, `get_board`,
  `list_cards`, `get_card`, `create_card`, `update_card`, `move_card`,
  `add_card_member`, `remove_card_member`, `list_comments`, `add_comment`,
  `list_checklists`, `add_checklist_item`, `set_checklist_item_checked`,
  `current_user`.
- Входы инструментов — типизированные структуры с `schemars::JsonSchema`
  (rmcp генерирует схемы автоматически); выход — JSON сущностей клиента.
- Рейтлимит прозрачен для агента: ретраи на 429 отрабатывают внутри
  kaiten-client. Только при исчерпании попыток инструмент возвращает ошибку
  с текстом вида `rate limited, retry after <n>s`, чтобы агент знал, что делать.
- Логи — только в stderr (stdout занят протоколом MCP).

## Отладка

- `-v` / `-vv` → уровни debug/trace в stderr; переменная `RUST_LOG` тоже работает.
- В debug видно каждый HTTP-запрос (метод, путь, статус, длительность),
  в trace — тела; токен редактируется всегда.
- Ошибки десериализации указывают точный путь до поля (`serde_path_to_error`).
- `kaiten api <METHOD> <path>` — сырой доступ к любому эндпоинту, когда
  типизированной команды не хватает; вывод — сырой JSON.
- Тела ошибок API печатаются как есть вместе с HTTP-статусом.

## Тестирование

- **kaiten-client**: wiremock-тесты на каждый метод — проверяется форма запроса
  (путь, метод, заголовок авторизации, тело), десериализация реалистичных
  JSON-фикстур, маппинг ошибок (4xx/5xx → `Api`), ретрай на 429.
- **CLI**: `assert_cmd` + wiremock (`KAITEN_BASE_URL` указывает на мок),
  снапшоты табличного вывода через `insta`, проверка `--json`-вывода,
  проверка exit-кодов на ошибках.
- **MCP**: in-process вызовы тул-хендлеров против wiremock; проверка, что все
  инструменты регистрируются и их схемы валидны.
- Фикстуры — `tests/fixtures/*.json`, снятые с реального API и обезличенные.
- Линты: `cargo clippy -- -D warnings`, `cargo fmt --check`.

## Критерии успеха

1. Повседневные операции (посмотреть свои карточки, создать, прокомментировать,
   передвинуть) выполняются из терминала быстрее, чем в веб-интерфейсе.
2. Агент через MCP выполняет те же операции без ручной помощи.
3. `cargo test` зелёный, каждый эндпоинт клиента и каждая команда CLI покрыты.
4. Любой сбой API диагностируется по `-vv`-логу или `kaiten api` без правки кода.
