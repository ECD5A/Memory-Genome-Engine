<h1 align="center">Memory Genome Engine</h1>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust 1.95+" src="https://img.shields.io/badge/Rust-1.95%2B-b45309?style=flat-square&logo=rust&logoColor=white"></a>
  <a href="LICENSE"><img alt="Apache 2.0" src="https://img.shields.io/badge/license-Apache--2.0-15803d?style=flat-square"></a>
  <a href="docs/ARCHITECTURE.md"><img alt="Local-first memory" src="https://img.shields.io/badge/local--first-memory-0e7490?style=flat-square"></a>
  <a href="docs/ARCHITECTURE.md"><img alt="Binary storage" src="https://img.shields.io/badge/binary-storage-6d28d9?style=flat-square"></a>
  <a href="docs/INTEGRATION.md"><img alt="CLI TUI MCP" src="https://img.shields.io/badge/CLI%20.%20TUI%20.%20MCP-0369a1?style=flat-square"></a>
  <a href="docs/SECURITY.md"><img alt="Encrypted stores" src="https://img.shields.io/badge/encrypted-stores-15803d?style=flat-square"></a>
  <a href="docs/INTEGRATION.md"><img alt="Python TypeScript SDK" src="https://img.shields.io/badge/Python%20.%20TypeScript-SDK-2563eb?style=flat-square"></a>
  <br>
  <sub><a href="https://github.com/ECD5A/Memory-Genome-Engine/blob/main/README.md">English version</a></sub>
</p>

Memory Genome Engine даёт локальным ИИ-агентам постоянную память проекта, доступную между рабочими сессиями без обязательного облачного сервиса или векторной базы. Движок хранит типизированные записи `MemoryCell`, описывает их через `MarkerGenome`, переносит неактивную память в запечатанные бинарные страницы и формирует `ContextPacket` для текущей задачи.

<p align="center">
  <img src="assets/mge-console-demo-ru.gif" alt="Терминальная панель Memory Genome Engine" width="100%">
</p>

## Возможности

- Сохраняет факты, решения, пользовательские предпочтения, заметки и результаты работы агентов.
- Держит активную память в быстром слое L1 Hot RAM и сохраняет её в бинарный журнал.
- Переносит накопленную память в неизменяемые запечатанные страницы с индексами кандидатов.
- Поддерживает режимы поиска `focused`, `broad` и `full-scope`.
- Импортирует существующие Markdown-заметки и позволяет безопасно менять статус записей.
- Предоставляет CLI, TUI, локальный MCP-совместимый stdio-сервер, Python SDK и TypeScript SDK.
- Поддерживает создаваемые по запросу зашифрованные хранилища для активных записей, снимков и содержимого запечатанных страниц.
- Использует только бинарные форматы во время работы; JSON применяется для протокола и отладочных отчётов.

## Зачем нужен MGE

MGE рассматривает память агента как проверяемую локальную подсистему, а не как непрозрачную историю чата или внешний поисковый сервис:

- `MarkerGenome` явно хранит область, тип, статус, доверие, чувствительность, тему и пользовательские маркеры.
- Цикл от горячей памяти к запечатанным страницам объединяет быстрый поиск в RAM с долговечным проверяемым бинарным хранилищем.
- `ContextPacket` возвращает ранжированную память вместе с ограничениями, предупреждениями и деталями оценки.
- CLI, MCP-совместимый stdio-сервер и тонкие SDK используют один Rust-движок без обязательного облака, embedding-модели или векторной базы.

Поиск по маркерам детерминирован и лёгок, но не заменяет универсальный семантический поиск. Важны опорные слова запроса и качество загрузки данных; production recall пока не применяет общий порог отказа от ответа. Отдельно приведены [измерения и их ограничения](docs/RELEASE.md#external-retrieval-evidence).

## Измеренная производительность

Воспроизводимый синтетический сценарий в оптимизированной сборке показывает базовые характеристики стандартного точного индекса:

| Метрика | Результат |
|---|---:|
| Набор данных | 1 280 записей / 80 запросов |
| Точность `focused`, Hit@5 / Recall@5 | 1.00 / 1.00 |
| Поиск в Hot RAM, среднее / p95 | 0.51 / 0.63 мс |
| Повторный поиск по запечатанным страницам, среднее / p95 | 0.27 / 0.38 мс |
| Холодное открытие хранилища и поиск, среднее | 2.40 мс |

Измерения выполнены при подключённом питании и плане Windows «Максимальная производительность» на Intel Core i7-9750H под Windows 10 x64, Rust 1.95.0, core commit `2fdbc99`, с пятью повторами. Это проверка корректности и скорости движка на синтетическом наборе, а не сравнение с конкурентами и не оценка качества ответа языковой модели. Методика и ограничения приведены в [документации по выпуску](docs/RELEASE.md#measured-engineering-baseline).

## Быстрый старт

Установите релиз с проверкой контрольной суммы по [инструкции быстрого старта](QUICKSTART.md), затем создайте хранилище и при необходимости подключите локальный агент:

```bash
mge setup
mge setup codex
mge remember "User prefers concise technical answers" --kind user_preference --scope global --trust user_confirmed
mge recall "How should the agent answer technical questions?"
mge seal
mge validate --deep
```

Терминальный интерфейс:

```bash
mge tui
mge setup --help
```

## Зашифрованное хранилище

```bash
export MGE_PASSPHRASE="use-a-real-secret"
mge init --encrypted --passphrase-env MGE_PASSPHRASE
mge remember "private memory" --passphrase-env MGE_PASSPHRASE
mge recall "private memory" --passphrase-env MGE_PASSPHRASE
mge seal --passphrase-env MGE_PASSPHRASE
mge validate --deep --passphrase-env MGE_PASSPHRASE
```

Шифрование содержимого защищает активные записи, снимки и содержимое запечатанных страниц. Словарь маркеров, индексы, сводные данные каталога, экспорт Markdown и память процесса после разблокировки остаются открытыми по принятой модели безопасности. Подробнее см. в разделе [«Модель безопасности»](docs/SECURITY.md).

## Интеграция с агентами

CLI:

```bash
mge recall "project context" --mode broad --scope my_project
```

MCP-совместимый stdio-сервер:

```bash
mge-mcp-server --store .memory-genome
```

Команды `mge setup codex`, `mge setup claude-code` и `mge setup cursor` автоматически регистрируют сервер. `mge setup generic-mcp` печатает переносимую конфигурацию для другого хоста. Подробности приведены в разделе [интеграции](docs/INTEGRATION.md).

Примеры SDK:

```bash
python examples/python_agent_host.py
node examples/typescript_agent_host.ts
```

## Документация

- [Быстрый старт](QUICKSTART.md)
- [Архитектура](docs/ARCHITECTURE.md)
- [Модель безопасности](docs/SECURITY.md)
- [Интеграция / MCP / SDK](docs/INTEGRATION.md)
- [Сборка и выпуск](docs/RELEASE.md)

## Сообщество

- [Лицензия](LICENSE)
- [Уведомление об авторских правах](NOTICE)
- [Политика безопасности](SECURITY.md)
- [Правила участия](CONTRIBUTING.md)
- [Кодекс поведения](CODE_OF_CONDUCT.md)

## Поддержать проект

Если Memory Genome Engine полезен для вашей работы, проект можно поддержать здесь:

- Bitcoin (BTC): `1ECDSA1b4d5TcZHtqNpcxmY8pBH1GgHntN`
- USDT (TRC20): `TUF4vPdB6QkjCvZq18rBL4Qj4dK5ihCN75`

## Контакты

Открыт к обсуждению коммерческой интеграции, поддержки, сотрудничества и партнёрства:

<p>
  <a href="mailto:stelmak159@gmail.com" aria-label="Email"><img alt="Email" height="24" src="https://cdn.simpleicons.org/gmail/EA4335"></a>
  &nbsp;
  <a href="https://t.me/ECDS4" aria-label="Telegram"><img alt="Telegram" height="24" src="https://cdn.simpleicons.org/telegram/26A5E4"></a>
  &nbsp;
  <a href="https://github.com/ECD5A/Memory-Genome-Engine" aria-label="GitHub repository"><picture><source media="(prefers-color-scheme: dark)" srcset="https://cdn.simpleicons.org/github/FFFFFF"><img alt="GitHub repository" height="24" src="https://cdn.simpleicons.org/github/181717"></picture></a>
</p>
