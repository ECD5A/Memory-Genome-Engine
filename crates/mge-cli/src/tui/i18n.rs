#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    En,
    Ru,
}

impl Language {
    pub fn toggle(self) -> Self {
        match self {
            Self::En => Self::Ru,
            Self::Ru => Self::En,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "EN",
            Self::Ru => "RU",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TKey {
    Subtitle,
    Dashboard,
    Menu,
    Store,
    RecallMemory,
    AddMemoryCell,
    SealHotMemory,
    StoreStatus,
    ExportImportMarkdown,
    Settings,
    SetupStore,
    Help,
    Exit,
    StorePath,
    HotCells,
    SealedPages,
    SealedCells,
    MarkerCount,
    IndexKind,
    DefaultRecallMode,
    ActiveLanguage,
    StorageSize,
    Health,
    Query,
    RecallMode,
    ResultLimit,
    Markers,
    Scope,
    Kind,
    RunRecall,
    Results,
    Diagnostics,
    EmptyResults,
    TitleSummary,
    Content,
    Status,
    Trust,
    Sensitivity,
    SaveMemory,
    SealAction,
    ConfirmSeal,
    SealResult,
    Refresh,
    DeepDoctor,
    ExportMarkdown,
    ImportMarkdown,
    ImportUnavailable,
    ExportPath,
    Language,
    TimingDiagnostics,
    DebugJsonOutput,
    MarkdownExport,
    ExperimentalFeatures,
    HumanDashboard,
    HelpText,
    HelpWorkflow,
    HelpLanguage,
    HelpSafety,
    FooterDashboard,
    FooterScreen,
    FooterSettings,
    TooSmall,
    Ok,
    Warn,
    Error,
    On,
    Off,
    Saved,
    NotInitialized,
    Unlocked,
    NoData,
    CreatedPages,
    ArchivedHotLog,
    CandidatePages,
    LoadedPages,
    ScannedCells,
    ResultCount,
    RecallLatency,
    Checkpoint,
    ValidateDeep,
    RebuildIndexes,
    ReadOnlyImportNote,
    ApplyIndexKind,
    OperationStatus,
    InitFastStore,
    InitEncryptedStore,
    SetupAlreadyInitialized,
    SetupReady,
    PassphraseEnv,
    MgeSetupCommand,
    McpSdkGuidance,
    MarkdownPlaintextWarning,
    EncryptedSetupHint,
    FirstRunHelp,
}

#[cfg(test)]
pub const ALL_KEYS: &[TKey] = &[
    TKey::Subtitle,
    TKey::Dashboard,
    TKey::Menu,
    TKey::Store,
    TKey::RecallMemory,
    TKey::AddMemoryCell,
    TKey::SealHotMemory,
    TKey::StoreStatus,
    TKey::ExportImportMarkdown,
    TKey::Settings,
    TKey::SetupStore,
    TKey::Help,
    TKey::Exit,
    TKey::StorePath,
    TKey::HotCells,
    TKey::SealedPages,
    TKey::SealedCells,
    TKey::MarkerCount,
    TKey::IndexKind,
    TKey::DefaultRecallMode,
    TKey::ActiveLanguage,
    TKey::StorageSize,
    TKey::Health,
    TKey::Query,
    TKey::RecallMode,
    TKey::ResultLimit,
    TKey::Markers,
    TKey::Scope,
    TKey::Kind,
    TKey::RunRecall,
    TKey::Results,
    TKey::Diagnostics,
    TKey::EmptyResults,
    TKey::TitleSummary,
    TKey::Content,
    TKey::Status,
    TKey::Trust,
    TKey::Sensitivity,
    TKey::SaveMemory,
    TKey::SealAction,
    TKey::ConfirmSeal,
    TKey::SealResult,
    TKey::Refresh,
    TKey::DeepDoctor,
    TKey::ExportMarkdown,
    TKey::ImportMarkdown,
    TKey::ImportUnavailable,
    TKey::ExportPath,
    TKey::Language,
    TKey::TimingDiagnostics,
    TKey::DebugJsonOutput,
    TKey::MarkdownExport,
    TKey::ExperimentalFeatures,
    TKey::HumanDashboard,
    TKey::HelpText,
    TKey::HelpWorkflow,
    TKey::HelpLanguage,
    TKey::HelpSafety,
    TKey::FooterDashboard,
    TKey::FooterScreen,
    TKey::FooterSettings,
    TKey::TooSmall,
    TKey::Ok,
    TKey::Warn,
    TKey::Error,
    TKey::On,
    TKey::Off,
    TKey::Saved,
    TKey::NotInitialized,
    TKey::Unlocked,
    TKey::NoData,
    TKey::CreatedPages,
    TKey::ArchivedHotLog,
    TKey::CandidatePages,
    TKey::LoadedPages,
    TKey::ScannedCells,
    TKey::ResultCount,
    TKey::RecallLatency,
    TKey::Checkpoint,
    TKey::ValidateDeep,
    TKey::RebuildIndexes,
    TKey::ReadOnlyImportNote,
    TKey::ApplyIndexKind,
    TKey::OperationStatus,
    TKey::InitFastStore,
    TKey::InitEncryptedStore,
    TKey::SetupAlreadyInitialized,
    TKey::SetupReady,
    TKey::PassphraseEnv,
    TKey::MgeSetupCommand,
    TKey::McpSdkGuidance,
    TKey::MarkdownPlaintextWarning,
    TKey::EncryptedSetupHint,
    TKey::FirstRunHelp,
];

pub fn tr(language: Language, key: TKey) -> &'static str {
    match language {
        Language::En => match key {
            TKey::Subtitle => "Local-first memory engine for AI agents by ECD5A",
            TKey::Dashboard => "Dashboard",
            TKey::Menu => "Menu",
            TKey::Store => "Store",
            TKey::RecallMemory => "Recall memory",
            TKey::AddMemoryCell => "Add memory cell",
            TKey::SealHotMemory => "Seal hot memory",
            TKey::StoreStatus => "Store status",
            TKey::ExportImportMarkdown => "Export / import Markdown",
            TKey::Settings => "Settings",
            TKey::SetupStore => "Setup store",
            TKey::Help => "Help",
            TKey::Exit => "Exit",
            TKey::StorePath => "Store path",
            TKey::HotCells => "Hot cells",
            TKey::SealedPages => "Sealed pages",
            TKey::SealedCells => "Sealed cells",
            TKey::MarkerCount => "Markers",
            TKey::IndexKind => "Index kind",
            TKey::DefaultRecallMode => "Default recall mode",
            TKey::ActiveLanguage => "Language",
            TKey::StorageSize => "Storage size",
            TKey::Health => "Health",
            TKey::Query => "Query",
            TKey::RecallMode => "Recall mode",
            TKey::ResultLimit => "Result limit",
            TKey::Markers => "Markers",
            TKey::Scope => "Scope",
            TKey::Kind => "Kind",
            TKey::RunRecall => "Run recall",
            TKey::Results => "Results",
            TKey::Diagnostics => "Diagnostics",
            TKey::EmptyResults => "No relevant memory found.",
            TKey::TitleSummary => "Title / summary",
            TKey::Content => "Content",
            TKey::Status => "Status",
            TKey::Trust => "Trust",
            TKey::Sensitivity => "Sensitivity",
            TKey::SaveMemory => "Save memory",
            TKey::SealAction => "Run seal",
            TKey::ConfirmSeal => "Press Enter again to seal hot memory.",
            TKey::SealResult => "Seal result",
            TKey::Refresh => "Refresh",
            TKey::DeepDoctor => "Deep doctor",
            TKey::ExportMarkdown => "Export Markdown",
            TKey::ImportMarkdown => "Import Markdown",
            TKey::ImportUnavailable => "Import is not supported yet.",
            TKey::ExportPath => "Export path",
            TKey::Language => "Language",
            TKey::TimingDiagnostics => "Timing diagnostics",
            TKey::DebugJsonOutput => "Debug JSON output",
            TKey::MarkdownExport => "Markdown export",
            TKey::ExperimentalFeatures => "Experimental features",
            TKey::HumanDashboard => "Human dashboard",
            TKey::HelpText => {
                "Use arrows to move, Enter to open or run, Space to toggle, Esc to go back."
            }
            TKey::HelpWorkflow => {
                "Recommended loop: recall context, do the work, remember useful results, checkpoint or seal."
            }
            TKey::HelpLanguage => "Switch language at any time with F1, L/l, or Д/д.",
            TKey::HelpSafety => {
                "The TUI calls the same local CLI/core paths; storage remains binary-first."
            }
            TKey::FooterDashboard => "↑↓ select  Enter open  F1/L/Д language  F2 help  q quit",
            TKey::FooterScreen => {
                "↑↓ select  Enter run/edit  Esc back  F1/L/Д language  F2 help"
            }
            TKey::FooterSettings => {
                "↑↓ select  ←→ change  Space toggle  Esc back  F1/L/Д language"
            }
            TKey::TooSmall => "Terminal is too small. Please enlarge it.",
            TKey::Ok => "OK",
            TKey::Warn => "WARN",
            TKey::Error => "ERROR",
            TKey::On => "ON",
            TKey::Off => "OFF",
            TKey::Saved => "saved",
            TKey::NotInitialized => "not initialized",
            TKey::Unlocked => "unlocked",
            TKey::NoData => "no data",
            TKey::CreatedPages => "Created pages",
            TKey::ArchivedHotLog => "Archived hot log",
            TKey::CandidatePages => "Candidate pages",
            TKey::LoadedPages => "Loaded pages",
            TKey::ScannedCells => "Scanned cells",
            TKey::ResultCount => "Result count",
            TKey::RecallLatency => "Recall latency",
            TKey::Checkpoint => "Checkpoint",
            TKey::ValidateDeep => "Validate deep",
            TKey::RebuildIndexes => "Rebuild indexes",
            TKey::ReadOnlyImportNote => "Markdown import is not implemented.",
            TKey::ApplyIndexKind => "Apply index kind",
            TKey::OperationStatus => "Operation status",
            TKey::InitFastStore => "Initialize fast local store",
            TKey::InitEncryptedStore => "Initialize encrypted store",
            TKey::SetupAlreadyInitialized => "Store is already initialized.",
            TKey::SetupReady => "Store is ready.",
            TKey::PassphraseEnv => "Passphrase env",
            TKey::MgeSetupCommand => "Use `mge setup` for non-interactive first-run setup.",
            TKey::McpSdkGuidance => {
                "Agent hosts can use `mge-mcp-server`; Python/TypeScript SDKs wrap the Rust CLI."
            }
            TKey::MarkdownPlaintextWarning => {
                "Markdown export is human-readable plaintext. Do not export secrets unless intended."
            }
            TKey::EncryptedSetupHint => {
                "Encrypted setup requires --passphrase-env. The passphrase value is read from the environment, not typed into the TUI."
            }
            TKey::FirstRunHelp => {
                "First run: initialize a store, add memory, recall it, then checkpoint or seal."
            }
        },
        Language::Ru => match key {
            TKey::Subtitle => "Local-first memory engine for AI agents by ECD5A",
            TKey::Dashboard => "Панель",
            TKey::Menu => "Меню",
            TKey::Store => "Хранилище",
            TKey::RecallMemory => "Вспомнить память",
            TKey::AddMemoryCell => "Добавить запись",
            TKey::SealHotMemory => "Запечатать hot memory",
            TKey::StoreStatus => "Статус хранилища",
            TKey::ExportImportMarkdown => "Экспорт / импорт Markdown",
            TKey::Settings => "Настройки",
            TKey::SetupStore => "Настроить хранилище",
            TKey::Help => "Помощь",
            TKey::Exit => "Выход",
            TKey::StorePath => "Путь хранилища",
            TKey::HotCells => "Hot cells",
            TKey::SealedPages => "Sealed pages",
            TKey::SealedCells => "Sealed cells",
            TKey::MarkerCount => "Маркеры",
            TKey::IndexKind => "Тип индекса",
            TKey::DefaultRecallMode => "Recall mode по умолчанию",
            TKey::ActiveLanguage => "Язык",
            TKey::StorageSize => "Размер",
            TKey::Health => "Состояние",
            TKey::Query => "Запрос",
            TKey::RecallMode => "Режим recall",
            TKey::ResultLimit => "Лимит результатов",
            TKey::Markers => "Маркеры",
            TKey::Scope => "Scope",
            TKey::Kind => "Kind",
            TKey::RunRecall => "Запустить recall",
            TKey::Results => "Результаты",
            TKey::Diagnostics => "Диагностика",
            TKey::EmptyResults => "Релевантная память не найдена.",
            TKey::TitleSummary => "Заголовок / summary",
            TKey::Content => "Содержимое",
            TKey::Status => "Статус",
            TKey::Trust => "Trust",
            TKey::Sensitivity => "Sensitivity",
            TKey::SaveMemory => "Сохранить память",
            TKey::SealAction => "Запустить seal",
            TKey::ConfirmSeal => {
                "Нажмите Enter ещё раз, чтобы запечатать hot memory."
            }
            TKey::SealResult => "Результат seal",
            TKey::Refresh => "Обновить",
            TKey::DeepDoctor => "Глубокий doctor",
            TKey::ExportMarkdown => "Экспорт Markdown",
            TKey::ImportMarkdown => "Импорт Markdown",
            TKey::ImportUnavailable => "Импорт пока не поддержан.",
            TKey::ExportPath => "Путь экспорта",
            TKey::Language => "Язык",
            TKey::TimingDiagnostics => "Timing diagnostics",
            TKey::DebugJsonOutput => "Debug JSON output",
            TKey::MarkdownExport => "Markdown export",
            TKey::ExperimentalFeatures => "Experimental features",
            TKey::HumanDashboard => "Human dashboard",
            TKey::HelpText => {
                "Стрелки двигают выбор, Enter открывает или запускает, Space переключает, Esc назад."
            }
            TKey::HelpWorkflow => {
                "Рабочий цикл: recall контекста, работа, remember полезного результата, checkpoint или seal."
            }
            TKey::HelpLanguage => "Язык переключается в любой момент через F1, L/l или Д/д.",
            TKey::HelpSafety => {
                "TUI использует тот же локальный CLI/core путь; runtime storage остаётся бинарным."
            }
            TKey::FooterDashboard => "↑↓ выбор  Enter открыть  F1/L/Д язык  F2 помощь  q выход",
            TKey::FooterScreen => {
                "↑↓ выбор  Enter запуск/редактирование  Esc назад  F1/L/Д язык  F2 помощь"
            }
            TKey::FooterSettings => {
                "↑↓ выбор  ←→ изменить  Space вкл/выкл  Esc назад  F1/L/Д язык"
            }
            TKey::TooSmall => "Терминал слишком маленький. Увеличьте окно.",
            TKey::Ok => "OK",
            TKey::Warn => "ВНИМАНИЕ",
            TKey::Error => "ОШИБКА",
            TKey::On => "ВКЛ",
            TKey::Off => "ВЫКЛ",
            TKey::Saved => "сохранено",
            TKey::NotInitialized => "не инициализировано",
            TKey::Unlocked => "открыто",
            TKey::NoData => "нет данных",
            TKey::CreatedPages => "Создано страниц",
            TKey::ArchivedHotLog => "Архив hot log",
            TKey::CandidatePages => "Candidate pages",
            TKey::LoadedPages => "Loaded pages",
            TKey::ScannedCells => "Scanned cells",
            TKey::ResultCount => "Результатов",
            TKey::RecallLatency => "Recall latency",
            TKey::Checkpoint => "Checkpoint",
            TKey::ValidateDeep => "Validate deep",
            TKey::RebuildIndexes => "Rebuild indexes",
            TKey::ReadOnlyImportNote => "Markdown import ещё не реализован.",
            TKey::ApplyIndexKind => "Применить тип индекса",
            TKey::OperationStatus => "Статус операции",
            TKey::InitFastStore => "Инициализировать быстрый локальный store",
            TKey::InitEncryptedStore => "Инициализировать encrypted store",
            TKey::SetupAlreadyInitialized => "Store уже инициализирован.",
            TKey::SetupReady => "Store готов.",
            TKey::PassphraseEnv => "Passphrase env",
            TKey::MgeSetupCommand => "Для неинтерактивного первого запуска используйте `mge setup`.",
            TKey::McpSdkGuidance => {
                "Agent hosts могут использовать `mge-mcp-server`; Python/TypeScript SDK оборачивают Rust CLI."
            }
            TKey::MarkdownPlaintextWarning => {
                "Markdown export - человекочитаемый plaintext. Не экспортируйте секреты без явного намерения."
            }
            TKey::EncryptedSetupHint => {
                "Encrypted setup требует --passphrase-env. Значение passphrase читается из environment, а не вводится в TUI."
            }
            TKey::FirstRunHelp => {
                "Первый запуск: инициализируйте store, добавьте память, выполните recall, затем checkpoint или seal."
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_translation_keys_are_present() {
        for key in ALL_KEYS {
            assert!(!tr(Language::En, *key).is_empty());
            assert!(!tr(Language::Ru, *key).is_empty());
        }
    }

    #[test]
    fn language_toggles_without_restart_state() {
        assert_eq!(Language::En.toggle(), Language::Ru);
        assert_eq!(Language::Ru.toggle(), Language::En);
    }

    #[test]
    fn russian_translations_do_not_contain_common_utf8_mojibake() {
        let suspicious = [
            "\u{0420}\u{203a}",
            "\u{0420}\u{045f}",
            "\u{0420}\u{201d}",
            "\u{0421}\u{045a}",
            "\u{0432}\u{2020}",
            "\u{0432}\u{2014}",
            "\u{0432}\u{045a}",
        ];
        for key in ALL_KEYS {
            let value = tr(Language::Ru, *key);
            for marker in suspicious {
                assert!(
                    !value.contains(marker),
                    "translation {:?} contains mojibake marker {:?}: {}",
                    key,
                    marker,
                    value
                );
            }
        }
    }
}
