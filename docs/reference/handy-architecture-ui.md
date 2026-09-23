# Handy: устройство приложения и UI — справочник паттернов

Снимок кода: upstream `cjpais/Handy` на коммите `8f9cf53` (релиз 0.9.7, 2026-09-19). На момент разбора ветка форка `satoru` совпадала с этим коммитом. Все ссылки вида `путь:строка` относятся к этому коммиту; после синхронизации с upstream номера строк поедут, поэтому рядом с каждой ссылкой в скобках стоит имя символа, по которому место находится поиском. Разбор выполнен 2026-09-24, задача beads `handy-dow`.

Handy — десктопное приложение для диктовки на Tauri 2: бэкенд на Rust, фронтенд на React 18, TypeScript, Zustand и Tailwind v4. Интерфейс у него маленький и аккуратный, а интеграция с ОС (глобальные шорткаты, трей, оверлей поверх всех окон, вставка текста в чужое приложение) сделана тщательно и с разбором платформенных багов. Справочник собирает приёмы, которые стоит переносить в свои проекты, и отдельно — места, которые переносить не надо (раздел 4).

Каждый паттерн описан по одной схеме: где лежит, как устроен, что в нём неочевидного, как перенести. Поведение описано по чтению кода; приложение для разбора не запускалось. Где вывод сделан только по чтению и может расходиться с рантаймом, это сказано явно.

## 0. Карта приложения

У приложения два окна, и оба создаются в Rust, а не в `tauri.conf.json`: там стоит пустой список `src-tauri/tauri.conf.json:14` (`"windows"`). Главное окно — настройки, точка входа `index.html` → `src/main.tsx`. Второе окно — оверлей записи, точка входа `src/overlay/index.html` → `src/overlay/main.tsx`. Vite собирает оба как отдельные входы: `vite.config.ts:21-28` (`overlay`).

Бэкенд держит четыре менеджера (`ModelManager`, `TranscriptionManager`, `AudioRecordingManager`, `HistoryManager`) как `Arc<T>` в состоянии Tauri. Над ними стоит `TranscriptionCoordinator` — единственная точка входа для команд «начать/остановить запись», откуда бы они ни пришли: шорткат, трей, CLI или сигнал. Вокруг — модули интеграции с ОС: два бэкенда глобальных шорткатов, трей, оверлей, вставка через буфер обмена, мониторинг Secure Input на macOS.

Фронтенд вызывает бэкенд через команды, типы и обёртки которых генерирует tauri-specta в `src/bindings.ts`. Бэкенд сообщает фронтенду об изменениях событиями; большинство событий строковые, типизированных три.

Одна диктовка проходит так:

1. Глобальный шорткат срабатывает в одном из двух бэкендов и попадает в `src-tauri/src/shortcut/handler.rs:29` (`handle_shortcut_event`), который отправляет нажатие в канал координатора.
2. Координатор решает, что запись надо начать, и вызывает `start()` действия из реестра `src-tauri/src/actions.rs:930` (`ACTION_MAP`). Менеджер записи открывает микрофон, параллельно менеджер транскрипции начинает загружать модель, оверлей получает `show-overlay`, после первых реальных сэмплов — `recording-ready`, дальше около 30 раз в секунду — уровни `mic-level`.
3. Отпускание клавиши или повторное нажатие приводит к `stop()`: асинхронная задача останавливает запись, распознаёт речь, при включённой опции прогоняет текст через LLM, вставляет результат через буфер обмена и сохраняет запись в историю. В конце гард `src-tauri/src/actions.rs:37` (`FinishGuard`) сообщает координатору, что обработка закончилась.
4. Ошибки на любом шаге уходят событиями `recording-error`, `paste-error`, `transcription-error` и показываются тостом в главном окне.

Изменение настройки из UI идёт по пути `updateSetting` в Zustand-сторе → сгенерированная команда `commands.changeXxxSetting` → Rust-сеттер, который читает настройки, меняет поле, записывает и выполняет побочные эффекты (трей, автозапуск, тема окна). Если настройку меняет сам бэкенд, он отправляет событие `settings-changed`, и фронтенд перечитывает настройки целиком.

## 1. Фронтенд

### 1.1 Оболочка: гейт онбординга

Где: `src/App.tsx:34` (`OnboardingStep`), `src/App.tsx:225` (`checkOnboardingStatus`), `src/App.tsx:82-86` (`data-onboarding-active`), `src/App.tsx:107` (`initializeEnigo`).

Шаг онбординга хранится как `"accessibility" | "model" | "done"`, начальное значение — `null`, и пока оно `null`, `App` ничего не рендерит. Так пользователь не видит на долю секунды не тот экран. `checkOnboardingStatus` читает настройки через `commands.getAppSettings()`: новый пользователь проходит разрешения, затем выбор модели; вернувшийся сразу попадает в `"done"`, если только у него не пропало разрешение ОС. В этом случае главное окно принудительно показывается, и пользователь проходит только шаг разрешений, выбор модели пропускается. Если сама проверка упала, приложение откатывается к полному онбордингу.

Инициализация нативных подсистем (эмуляция клавиатуры через Enigo, регистрация глобальных шорткатов) запускается эффектом только в шаге `"done"` и защищена ref-флагом от повторного запуска. Атрибут `data-onboarding-active` на корневом элементе включает `scrollbar-gutter: stable both-edges`, чтобы центрированный онбординг не смещался, когда системные полосы прокрутки занимают место.

Перенос: держать явный автомат шагов онбординга, рендерить пустоту, пока шаг не известен, и привязывать к шагу `"done"` запуск всего, что вызывает системные запросы разрешений.

### 1.2 Реестр секций сайдбара

Где: `src/components/Sidebar.tsx:34` (`SECTIONS_CONFIG`), `src/components/Sidebar.tsx:17` (`SidebarSection`), `src/App.tsx:39-50` (`renderSettingsContent`).

Секции настроек описаны одним объектом `{labelKey, icon, component, enabled(settings)}`, объявленным как `as const satisfies Record<string, SectionConfig>`. Тип идентификатора секции выводится из ключей объекта (`keyof typeof SECTIONS_CONFIG`), поэтому новая секция регистрируется одной записью, и опечатка в идентификаторе ловится компилятором. Функция `enabled` решает видимость: секция постобработки видна только при `post_process_enabled`, секция отладки — только при `debug_mode`. `App` берёт `SECTIONS_CONFIG[section].component` и при неизвестном ключе показывает `general`.

Неочевидное: поле `component` типизировано как компонент без пропсов, поэтому страница отладки, которой нужен колбэк `onPreviewOnboarding`, обрабатывается отдельной веткой в `src/App.tsx:43-45`. Текущая секция не сбрасывается, когда становится скрытой: по чтению кода, если выключить debug-режим, находясь на странице отладки, страница останется открытой без подсвеченного пункта в сайдбаре.

Перенос: реестр страниц как `const satisfies` даёт типобезопасные идентификаторы и видимость по условию; стоит сразу предусмотреть передачу пропсов страницам и сброс на первую видимую секцию.

### 1.3 Раскладка окна и единственный скролл-контейнер

Где: `src/App.tsx:89-91` (`scrollTo`), `src/components/footer/Footer.tsx:7-41` (`Footer`).

Корень окна — `h-screen flex flex-col select-none cursor-default`. Внутри строка из сайдбара шириной `w-40` и колонки контента; в колонке прокручивается ровно один элемент с `flex-1 overflow-y-auto`, и при смене секции `useLayoutEffect` сбрасывает его прокрутку в ноль до отрисовки кадра. Над контентом любой секции выводятся баннеры о разрешениях и Secure Input (раздел 3.16). Футер стоит вне прокрутки: слева переключатель модели со статусом, справа строка обновлений и версия. Выделение текста выключено на всё приложение и включается точечно классом `select-text`, например в отображении путей.

Перенос: один скролл-контейнер на панель и сброс прокрутки в `useLayoutEffect` по ключу секции, а не в `useEffect`, чтобы не было видно прыжка.

### 1.4 Композиция страницы настроек

Где: `src/components/settings/general/GeneralSettings.tsx:16-43` (`SettingsGroup`), `src/components/settings/advanced/AdvancedSettings.tsx:30` (`experimental_enabled`).

Каждая страница — `<div className="max-w-3xl w-full mx-auto space-y-6">` с несколькими `SettingsGroup`, внутри которых стоят компоненты отдельных настроек с пропсами `descriptionMode="tooltip"` и `grouped={true}`. Страница только раскладывает, группа только рисует карточку, логика живёт в компонентах настроек. Условия показа тоже решаются на странице: группа экспериментальных настроек появляется при `experimental_enabled`, шорткат отмены скрыт на Linux, строки выбора устройства вывода и громкости получают `disabled`, когда звуковая обратная связь выключена.

Неочевидное: `SettingsGroup` не умеет кнопку в заголовке, поэтому страница истории копирует разметку группы вручную (`src/components/settings/history/HistorySettings.tsx:269-285`).

Перенос: трёхуровневая схема «страница → группа → настройка» позволяет переставлять настройки между страницами, не трогая логику.

### 1.5 Идиома «один файл — одна настройка»

Где: `src/components/settings/AudioFeedback.tsx:13` (`AudioFeedback`), `src/components/settings/ShowOverlay.tsx:13` (`ShowOverlay`), `src/components/settings/debug/PasteDelay.tsx:6-46` (`PasteDelay`), `src/components/settings/MicrophoneSelector.tsx:13-76` (`onRefresh`).

Компонент настройки — `React.memo(({ descriptionMode = "tooltip", grouped = false }) => …)`. Он берёт из хука `useSettings()` три функции `getSetting`, `updateSetting`, `isUpdating`, рендерит один контрол UI-кита, а подпись и описание берёт из ключей `settings.<область>.<ключ>.label|description`. Переключатели используют `ToggleSwitch`, который уже содержит строку настройки; остальные контролы (`Dropdown`, `Input`) оборачиваются в `SettingContainer`. `isUpdating(key)` передаётся в `disabled` или в спиннер контрола.

```tsx
// src/components/settings/AudioFeedback.tsx
const { getSetting, updateSetting, isUpdating } = useSettings();
const audioFeedbackEnabled = getSetting("audio_feedback") || false;
return (
  <ToggleSwitch
    checked={audioFeedbackEnabled}
    onChange={(enabled) => updateSetting("audio_feedback", enabled)}
    isUpdating={isUpdating("audio_feedback")}
    label={t("settings.sound.audioFeedback.label")}
    description={t("settings.sound.audioFeedback.description")}
    descriptionMode={descriptionMode}
    grouped={grouped}
  />
);
```

Один компонент может давать несколько строк: `ShowOverlay` показывает строку позиции, только когда стиль оверлея не `"none"`. Компонент можно параметризовать ключом: `PasteDelay` принимает `settingKey`, `labelKey`, `descriptionKey` и смонтирован дважды для двух разных задержек (`src/components/settings/debug/DebugSettings.tsx:49-56`, `PasteDelay`). `MicrophoneSelector` ставит рядом с выпадающим списком кнопку сброса к значению по умолчанию и через `onRefresh` перечитывает список устройств в момент открытия меню.

Перенос: единый контракт пропсов `{descriptionMode, grouped}` для всех компонентов настроек делает их взаимозаменяемыми блоками.

### 1.6 Строка настройки и карточка группы

Где: `src/components/ui/SettingContainer.tsx:4-13` (`SettingContainerProps`), `src/components/ui/SettingContainer.tsx:15` (`SettingContainer`), `src/components/ui/SettingsGroup.tsx:9-31` (`divide-y`).

`SettingContainer` рисует одну строку в четырёх вариантах: раскладка `horizontal` (по умолчанию) или `stacked`, описание `tooltip` (по умолчанию) или `inline`. Горизонтальная строка — `flex items-center justify-between min-h-12 px-4 p-2`, блок заголовка ограничен `max-w-2/3`. В режиме подсказки рядом с заголовком стоит иконка «i»: она фокусируется с клавиатуры (`role="button"`, `tabIndex=0`, Enter и Space), открывает подсказку по наведению или клику и закрывается кликом снаружи. Флаг `grouped` убирает у строки собственную рамку и скругление; `SettingsGroup` рисует одну карточку `bg-background border rounded-lg` с разделителями `divide-y divide-mid-gray/20` и необязательным заголовком `text-xs uppercase text-mid-gray`.

Неочевидное: `disabled` только приглушает заголовок и описание (`opacity-50`), сам контрол надо отключать отдельно. В варианте `stacked` + `tooltip` позиция подсказки зашита как `top` и проп `tooltipPosition` игнорируется.

Перенос: флаг `grouped`, снимающий рамку со строки, плюс группа с `divide-y` дают из одного компонента и одиночные строки, и сгруппированные карточки.

### 1.7 UI-кит

Где: `src/components/ui/`. Своих примитивов немного, внешних зависимостей две: `react-select` для одного сложного списка и `lucide-react` для иконок. Radix, Headless UI и подобных библиотек нет.

| Компонент                         | Основные пропсы                                                                                                           | Что стоит знать                                                                                                                                            |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ToggleSwitch`                    | `checked`, `onChange`, `label`, `description`, `isUpdating`, `descriptionMode`, `grouped`                                 | Скрытый checkbox со стилями через `peer-*`; уже содержит `SettingContainer`; при `isUpdating` показывает спиннер и блокируется.                            |
| `Slider`                          | `value`, `onChange`, `min`, `max`, `step`, `showValue`, `formatValue`, `onReset`                                          | Заливка трека — инлайн-градиент на токене `--color-background-ui`; при `onReset` рядом появляется кнопка сброса.                                           |
| `Button`                          | `variant`: `primary`, `primary-soft`, `secondary`, `warning`, `danger`, `danger-ghost`, `ghost`; `size`: `sm`, `md`, `lg` | Прокидывает все атрибуты `<button>`.                                                                                                                       |
| `Input`, `Textarea`               | `variant`: `default`, `compact`                                                                                           | У `Textarea` минимальная высота 100 или 80 px и `resize-y`.                                                                                                |
| `Dropdown`                        | `options`, `selectedValue`, `onSelect`, `onRefresh`                                                                       | Свой компонент: кнопка и абсолютно позиционированное меню `z-50 max-h-60`, закрытие кликом снаружи. Используется в 21 файле.                               |
| `Select`                          | `isCreatable`, `onCreateOption`, `isClearable`                                                                            | Обёртка `react-select`; цвета через `color-mix(in srgb, var(--color-…) N%, transparent)`, чтобы совпадать с Tailwind-прозрачностью `/N`. Один потребитель. |
| `Dialog`                          | `open`, `onOpenChange`, `title`, `closeLabel`, `footer`, `dismissible`, `initialFocusRef`                                 | Написан вручную, около 200 строк (раздел 1.9).                                                                                                             |
| `Tooltip`                         | `targetRef`, `position`                                                                                                   | Портал в `document.body` (раздел 1.8).                                                                                                                     |
| `ResetButton`                     | `onClick`, `disabled`, `ariaLabel`                                                                                        | Иконка сброса рядом с контролом, без подтверждения.                                                                                                        |
| `PathDisplay`                     | `path`, `onOpen`                                                                                                          | Моноширинный выделяемый путь и кнопка «Open».                                                                                                              |
| `AudioPlayer`, `AudioPlayerGroup` | `onLoadRequest`                                                                                                           | Ленивая загрузка и один играющий плеер на группу (раздел 3.11).                                                                                            |
| `Badge`, `Alert`                  | `variant`                                                                                                                 | Пока используют сырые цвета Tailwind, а не токены темы.                                                                                                    |

Неочевидное: `src/components/ui/index.ts` экспортирует только 9 из 17 компонентов, остальные импортируются по прямому пути, и в одном файле встречаются оба стиля импорта. `TextDisplay` экспортируется, но нигде не используется и содержит захардкоженные английские строки.

Перенос: для приложения настроек хватает десятка своих примитивов; внешняя библиотека нужна только там, где требуется поиск или создание значений в списке.

### 1.8 Подсказка через портал

Где: `src/components/ui/Tooltip.tsx:25` (`Tooltip`), `src/components/ui/Tooltip.tsx:93` (`createPortal`).

Подсказка рендерится порталом в `document.body` с `position: fixed`, шириной 200 px и `z-index` 9999, поэтому её не обрезает `overflow-hidden` у карточек. Если сверху или снизу меньше 12 px места, она переворачивается на другую сторону, по горизонтали прижимается к краям окна, а стрелка держится не ближе 12 px от края подсказки. Позиция пересчитывается на прокрутке (слушатель в фазе захвата) и на изменении размера окна. До первого измерения подсказка стоит в `-9999` с нулевой прозрачностью, чтобы не мигать в углу.

Перенос: портал с `fixed` и ручным пересчётом покрывает потребности окна настроек без библиотеки позиционирования.

### 1.9 Доступный диалог без зависимостей

Где: `src/components/ui/Dialog.tsx:46` (`Dialog`), `src/components/ui/Dialog.tsx:5-12` (`FOCUSABLE_SELECTOR`), `src/components/ui/Dialog.tsx:94-137` (`Escape`).

При открытии диалог запоминает элемент в фокусе, блокирует прокрутку `body` и на следующем кадре ставит фокус на `initialFocusRef` или на панель; при закрытии всё возвращается. Escape закрывает, Tab зациклен внутри панели по селектору фокусируемых элементов. Клик по подложке закрывает диалог по `onMouseDown`, и только если нажатие пришлось на саму подложку, а не на панель; так выделение текста, начатое внутри и отпущенное снаружи, диалог не закрывает. Разметка — `role="dialog"`, `aria-modal`, `aria-labelledby` и `aria-describedby` через `useId`. Проп `contentFades` добавляет градиентную маску на края прокручиваемого тела.

Перенос: этот файл можно брать почти целиком как эталон модального окна с правильной работой фокуса.

### 1.10 Тема: токены, Tailwind v4 и синхронизация окон

Где: `src/styles/theme.css:3-77` (`--light-color-text`), `src/styles/theme.css:61-64` (`data-theme="light"`), `src/App.css:6-16` (`@theme inline`), `src/lib/utils/theme.ts:27` (`applyTheme`), `src/lib/utils/theme.ts:54` (`syncThemeFromSettings`), `src-tauri/src/shortcut/mod.rs:586` (`change_theme_setting`), `src-tauri/src/shortcut/mod.rs:617` (`apply_window_theme`).

Каждый цвет объявлен один раз парой `--light-*` / `--dark-*`. Активные переменные `--color-*` по умолчанию светлые, переключаются на тёмные в `@media (prefers-color-scheme: dark)` и принудительно задаются селектором `:root[data-theme="light|dark"]`. Селектор атрибута специфичнее медиазапроса, поэтому явный выбор пользователя побеждает системную тему без `!important`. Файла `tailwind.config` нет: в `App.css` блок `@theme inline` регистрирует эти переменные как утилиты Tailwind, отсюда классы `bg-background`, `text-text`, `border-mid-gray/20`, `bg-logo-primary/80`.

```css
/* src/styles/theme.css */
:root[data-theme="light"] {
  --color-text: var(--light-color-text);
  --color-background: var(--light-color-background);
}
/* src/App.css */
@theme inline {
  --color-text: var(--color-text);
  --color-background: var(--color-background);
}
```

`applyTheme` ставит или удаляет `document.documentElement.dataset.theme` и дублирует выбор в `localStorage` под ключом `handy.theme`. Каждое окно применяет сохранённое значение до рендера React (`src/main.tsx:15-20`, `applyTheme`), поэтому вспышки неправильной темы нет, а затем сверяет его с настройками бэкенда. При смене темы `ThemeSelector` сначала применяет её локально, затем вызывает `updateSetting("theme")`; Rust сохраняет настройку, вызывает `window.set_theme` (это меняет заголовок окна на Windows и macOS) и рассылает событие `theme-changed` (`src-tauri/src/shortcut/mod.rs:603`, `theme-changed`), которое слушает окно оверлея (`src/overlay/main.tsx:19`, `theme-changed`).

Визуальные константы. Акцент — розовый логотип `#faa2ca` в светлой теме и `#f28cbb` в тёмной: кольца фокуса, активный пункт сайдбара. Насыщенный `--color-background-ui` `#da5893` — основная кнопка, включённый переключатель, заливка слайдера. Фон `#fbfbfb` / `#2c2b29`. Все рамки — один серый `--color-mid-gray` `#808080` с разной прозрачностью. Шрифт не задан, работает системный sans-стек Tailwind; базовый размер 15 px при межстрочном 24 px. Строки настроек `px-4 p-2 min-h-12`, страницы `space-y-6`, контейнеры `rounded-lg`, поля ввода `rounded-md`. Атрибут `data-platform` на корне позволяет писать платформенный CSS.

Неочевидное: вариант `dark:` в Tailwind не переопределён через `@custom-variant`, поэтому он следует системной теме, а не `data-theme`. На macOS это незаметно, потому что `set_theme` меняет и цветовую схему webview, а на Windows и Linux расходится; сейчас `dark:` используется только в просмотрщике логов. Часть розовых оттенков захардкожена мимо токенов (`src/components/ui/AudioPlayer.tsx:312`, `#FAA2CA`).

Перенос: пары `--light-*`/`--dark-*`, переключатель через атрибут на `<html>`, применение из `localStorage` до рендера и событие Tauri для остальных окон. Если используется `dark:`, сразу объявить `@custom-variant dark` на тот же атрибут.

### 1.11 Отдельные окна как отдельные входы Vite

Где: `vite.config.ts:21-28` (`overlay`), `src-tauri/src/overlay.rs:452` (`create_recording_overlay`).

Каждое окно — свой HTML-вход со своим `main.tsx`, которое само инициализирует i18n и тему. Окна создаются из Rust через `WebviewWindowBuilder` (на macOS — через `PanelBuilder` из `tauri-nspanel`). Общей памяти JS между окнами нет, состояние синхронизируется событиями Tauri. Оверлей вообще не использует Tailwind: его CSS импортирует только `theme.css` и объявляет свои токены `--s-*`. Алиасы путей: `@` → `src`, `@/bindings` → `src/bindings.ts`.

Перенос: один вход Vite на окно, создание окон в Rust, синхронизация через события, а не через общий стор.

### 1.12 RTL через логические утилиты

Где: `src/lib/utils/rtl.ts:55` (`initializeRTL`), `src/i18n/languages.ts:10` (`LANGUAGE_METADATA`), `src/components/ui/ToggleSwitch.tsx:47` (`rtl:`).

Метаданные языков хранят направление письма (арабский и иврит — `rtl`), и при смене языка на `<html>` выставляются `dir` и `lang`. Вёрстка использует логические утилиты: `border-e` вместо `border-r`, `end-4`, `text-start`, а в переключателе — `after:start-[2px]` с отдельным `rtl:`-сдвигом для включённого состояния.

Перенос: писать `start`/`end` с первого дня; переделка на RTL потом стоит дороже.

### 1.13 Стор настроек: карта обновителей, оптимистичное обновление, откат

Где: `src/stores/settingsStore.ts:82` (`settingUpdaters`), `src/stores/settingsStore.ts:313` (`updateSetting`), `src/stores/settingsStore.ts:345` (`resetSetting`), `src/stores/settingsStore.ts:356` (`updateBinding`), `src/hooks/useSettings.ts:48` (`useSettingsStore()`).

`settingUpdaters` — отображённый тип `{[K in keyof Settings]?: (value: Settings[K]) => Promise<unknown>}`, который сопоставляет каждому ключу настроек сгенерированную команду. Благодаря ему одна обобщённая функция `updateSetting(key, value)` обслуживает все настройки с проверкой типа значения. Она запоминает старое значение, ставит флаг `isUpdating[key]`, сразу записывает новое значение в стор, ждёт команду и при исключении возвращает старое; флаг снимается в `finally`. `resetSetting` берёт значение из `defaultSettings`, которые загружены из Rust, поэтому платформенные значения по умолчанию остаются правильными.

```ts
// src/stores/settingsStore.ts
const settingUpdaters: {
  [K in keyof Settings]?: (value: Settings[K]) => Promise<unknown>;
} = {
  audio_feedback: (value) =>
    commands.changeAudioFeedbackSetting(value as boolean),
  // …
};
```

Неочевидное, и это надо исправлять при переносе. Сгенерированные команды не бросают исключение при ошибке бэкенда, а возвращают `{status: "error"}`; большинство обновителей результат не проверяют, поэтому откат срабатывает только на сбой IPC. Проверяют результат лишь `selected_channel` и `vad_backend` (`src/stores/settingsStore.ts:111-118`, `selected_channel`). Откат пишет `{...settings, [key]: originalValue}` поверх снимка всего объекта, сделанного до вызова, и тем самым откатывает любые другие ключи, изменённые за время запроса; `updateBinding` делает это правильно, через функциональный `set`. Хук `useSettings` вызывает стор без селектора, поэтому каждый компонент настройки перерисовывается при любом изменении стора, включая чужие флаги `isUpdating`, и `React.memo` от этого не спасает. Компоненты, которым важна производительность, выбирают поле из стора напрямую (`src/App.tsx:66`, `useSettingsStore`).

Перенос: брать идею типизированной карты обновителей, но добавить помощник `unwrap(result)`, превращающий `{status: "error"}` в исключение, делать откат функциональным `set`, а в хуке отдавать селекторы.

### 1.14 Бэкенд как источник истины

Где: `src/stores/settingsStore.ts:630` (`initialize`), `src/stores/settingsStore.ts:228` (`refreshSettings`), `src/stores/settingsStore.ts:654` (`settings-changed`), `src/stores/modelStore.ts:268` (`initialize`), `src/stores/modelStore.ts:22` (`Immer`).

Стор настроек — кэш. При инициализации он параллельно загружает значения по умолчанию, текущие настройки, пользовательские звуки и признак блокировки обновлений, после чего подписывается на `model-state-changed` и `settings-changed`. Оба события вызывают полное перечитывание настроек; если изменился микрофон, дополнительно перечитывается список устройств. Бэкенд отправляет `settings-changed`, когда меняет настройку сам: откат бэкенда шорткатов, автоматический выбор микрофона после отключения устройства, переключение debug-режима. Полезная нагрузка — только имя настройки, без значения. Список аудиоустройств стор настроек при старте намеренно не запрашивает, чтобы не вызвать системный запрос доступа к микрофону до онбординга.

Стор моделей хранит состояние по идентификатору модели в обычных объектах (`downloadingModels`, `downloadProgress`, `extractingModels`), а не в `Set`/`Map`, чтобы Immer мог их менять через `produce`. Команды только запускают долгие операции; прогресс, завершение, ошибки, распаковка и удаление приходят событиями `model-download-*`, `model-extraction-*`, `model-deleted`, `models-updated`. Стор моделей инициализируется при загрузке модуля (`src/main.tsx:27`, `initialize`) и защищён флагом от повторного запуска; у стора настроек такого флага нет, и, по чтению кода, два компонента, смонтированные до окончания загрузки, могут зарегистрировать слушатели дважды.

Перенос: после изменения на стороне бэкенда отправлять короткий сигнал «изменилось X» и перечитывать состояние целиком, а не пересылать диффы; для долгих операций — команда запускает, события сообщают.

### 1.15 Типизированный IPC через tauri-specta

Где: `src-tauri/src/lib.rs:649` (`Builder::<tauri::Wry>::new()`), `src-tauri/src/lib.rs:768` (`collect_events!`), `src-tauri/src/lib.rs:774-780` (`export`), `src-tauri/src/lib.rs:897` (`mount_events`), `src/bindings.ts:1197` (`Result`), `src-tauri/Cargo.toml:86-88` (`tauri-specta`).

Все команды помечены сразу `#[tauri::command]` и `#[specta::specta]` и собраны одним `collect_commands!`. Один и тот же построитель tauri-specta выдаёт обработчик вызовов для Tauri и генерирует `src/bindings.ts`. Генерация выполняется только в отладочной сборке, `BigInt` экспортируется как `number`, файл коммитится в репозиторий и исключён из Prettier. Версии зафиксированы точно: `specta =2.0.0-rc.22`, `tauri-specta =2.0.0-rc.21`.

```rust
// src-tauri/src/lib.rs
#[cfg(debug_assertions)] // <- Only export on non-release builds
specta_builder
    .export(
        Typescript::default().bigint(BigIntExportBehavior::Number),
        "../src/bindings.ts",
    )
    .expect("Failed to export typescript bindings");
```

Команда, возвращающая `Result<T, String>`, превращается в обёртку, которая ловит отказ вызова: настоящий JS `Error` (сбой IPC) пробрасывается, а ошибка бэкенда возвращается значением `{status: "error", error}`. Команда без `Result` возвращает `Promise<T>` напрямую. Поэтому в местах вызова нужны и проверка `result.status === "ok"`, и `try/catch` (`src/stores/modelStore.ts:148-159`, `setActiveModel`).

Неочевидное: `bindings.ts` обновляется только запуском `tauri dev`; если поменять Rust и не запустить отладочную сборку, закоммиченный файл отстанет.

Перенос: один построитель на команды и события, фиксированные rc-версии, генерация как побочный эффект отладочного запуска и сгенерированный файл в git.

### 1.16 События: типизированные и строковые

Где: `src/bindings.ts:929` (`events`), `src-tauri/src/managers/history.rs:42-44` (`HistoryUpdatePayload`), `src-tauri/src/managers/transcription.rs:63-64` (`StreamTextEvent`), `src/overlay/RecordingOverlay.tsx:114` (`streamTextEvent`), `src/App.tsx:164-166` (`unlisten`).

Типизированных событий три. Структура выводит `tauri_specta::Event`, имя события получается из имени типа в kebab-case (`history-update-payload`), на стороне TypeScript доступны `events.x.listen`, `once` и `emit` с проверкой типов, а `events.x(window)` ограничивает событие одним окном. Перечисление с `#[serde(tag = "action")]` становится в TypeScript размеченным объединением `{action: "added", entry} | …`, что удобно для ленты изменений истории.

Остальные события строковые: `app.emit("name", payload)` в Rust и `listen<T>("name", cb)` в TypeScript с типами, продублированными вручную в `src/lib/types/events.ts`. Отписка в компонентах — `return () => { unlisten.then((fn) => fn()); }`. Высокочастотные события для одного окна отправляются через `emit_to("recording_overlay", …)`, а не широковещательно.

| Событие                                                        | Где отправляется                                                      | Где слушается                                                                                                                                         |
| -------------------------------------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `model-state-changed`                                          | `src-tauri/src/managers/transcription.rs:428` (`model-state-changed`) | `src/App.tsx:199`, стор настроек, стор моделей, переключатель модели; в Rust — `src-tauri/src/lib.rs:361` (`model-state-changed`), перестраивает трей |
| `model-download-progress`                                      | менеджер моделей и загрузчик                                          | `src/stores/modelStore.ts:277` (`model-download-progress`)                                                                                            |
| `settings-changed`                                             | сеттеры в `shortcut/mod.rs`, `managers/audio.rs`                      | `src/stores/settingsStore.ts:654` (`settings-changed`)                                                                                                |
| `recording-error`, `paste-error`, `transcription-error`        | `src-tauri/src/actions.rs`                                            | `src/App.tsx:143` (`recording-error`) и соседние эффекты                                                                                              |
| `show-overlay`, `hide-overlay`, `recording-ready`, `mic-level` | `src-tauri/src/overlay.rs`                                            | `src/overlay/RecordingOverlay.tsx:97` (`recording-ready`) и соседние                                                                                  |
| `theme-changed`                                                | `src-tauri/src/shortcut/mod.rs:603` (`theme-changed`)                 | `src/overlay/main.tsx:19` (`theme-changed`)                                                                                                           |

Перенос: всё, у чего есть полезная нагрузка, делать типизированными событиями; ручные копии типов расходятся с Rust незаметно.

### 1.17 i18n

Где: `src/i18n/index.ts:13-16` (`import.meta.glob`), `src/i18n/index.ts:103` (`syncLanguageFromSettings`), `src/i18n/index.ts:55` (`getSupportedLanguage`), `eslint.config.js:20` (`i18next/no-literal-string`), `scripts/check-translations.ts:9` (`REFERENCE_LANG`).

Локали лежат в `src/i18n/locales/<код>/translation.json` и подхватываются через `import.meta.glob`, поэтому новый язык добавляется новой папкой. i18next стартует с `lng: "en"`, `fallbackLng: "en"`, `useSuspense: false`, затем читает `app_language` из настроек бэкенда и переключается. `getSupportedLanguage` нормализует `_` в `-`, ищет точное совпадение и затем применяет правила для письменностей и регионов китайского. Язык интерфейса хранится в настройках Rust, а не в `localStorage`, поэтому главное окно, оверлей и трей читают одно значение; значение по умолчанию бэкенд берёт из локали ОС (`src-tauri/src/settings.rs:636`, `default_app_language`).

Две проверки держат переводы в порядке. ESLint-правило `i18next/no-literal-string` в режиме `markupOnly` запрещает текст в JSX, игнорируя `className`, `style`, `data-*`, `aria-*` и подобные атрибуты. Скрипт `check-translations.ts` сравнивает деревья ключей каждой локали с английской и падает в CI при недостающих или лишних ключах. Плейсхолдеры вроде `{{model}}` не сверяются.

Строки нативного меню трея генерируются из тех же JSON на этапе сборки: `src-tauri/build.rs:284` (`generate_tray_translations`) читает объект `tray` из каждой локали, превращает ключи в поля структуры `TrayStrings` и пишет таблицу в `OUT_DIR`, откуда её подключает `src-tauri/src/tray_i18n.rs:19` (`include!`). Отсутствующий в локали ключ превращается в пустую строку.

Перенос: единый источник переводов для веба и нативных поверхностей через генерацию в `build.rs`, lint-запрет литералов и проверка паритета ключей в CI.

### 1.18 Тесты фронтенда

Где: `playwright.config.ts:3` (`defineConfig`), `package.json:17` (`test:keyboard`).

Playwright гоняет Chromium против `vite dev` на порту 1420; в `tests/app.spec.ts` два дымовых теста. Vitest и Jest нет: модульные тесты — скрипты на `node:assert`, запускаемые через `bun <файл>`. Неочевидное: в `package.json` и CI подключён только `src/lib/utils/keyboard.test.ts`; `portableInstaller.test.ts` и `history/clipboard.test.ts` лежат в дереве, но автоматически не запускаются. Основная масса тестов — Rust-тесты в `src-tauri`.

## 2. Бэкенд

### 2.1 Запуск в четыре фазы

Где: `src-tauri/src/main.rs:8` (`CliArgs::parse()`), `src-tauri/src/lib.rs:623` (`run`), `src-tauri/src/lib.rs:889` (`.setup`), `src-tauri/src/lib.rs:1092` (`run`).

Первая фаза — процесс до Tauri: `main` разбирает аргументы clap и выставляет платформенные переменные окружения (на Linux `WEBKIT_DISABLE_DMABUF_RENDERER`, на Windows `VK_LOADER_LAYERS_DISABLE`), `run` включает аллокатор и портативный режим. Вторая — построитель: фильтр событий устройств и плагины в порядке dialog, log, nspanel (только macOS), single-instance (кроме headless-режима), fs, process, updater, os, clipboard-manager, macos-permissions, opener, store, global-shortcut, autostart. Третья — `setup`: монтирование типизированных событий, headless-ветка, скрытое главное окно, чтение настроек, применение `--debug`, координатор, менеджеры, монитор Secure Input, кэш оверлея, фоновый прогрев ускорителя, решение показать или спрятать окно. Четвёртая — цикл событий: `RunEvent::Reopen` пересоздаёт трей и показывает окно, `RunEvent::Exit` выгружает модель.

Перенос: явное разделение на «окружение процесса → плагины → состояние и окна → события цикла» упрощает поиск, где что инициализируется.

### 2.2 Менеджеры в состоянии Tauri

Где: `src-tauri/src/lib.rs:186` (`initialize_core_logic`), `src-tauri/src/lib.rs:216-220` (`manage`), `src-tauri/src/managers/transcription.rs:283` (`fn new`).

Каждый менеджер строится функцией `new(&AppHandle, зависимости…) -> anyhow::Result<Self>`, хранит клон `AppHandle` и читает настройки через `get_settings(app)`. Порядок построения следует зависимостям: модели → транскрипция (получает `Arc<ModelManager>`) → запись звука (получает маршрутизатор потока из транскрипции) → история. Каждый менеджер оборачивается в `Arc` и регистрируется `app_handle.manage(x.clone())`; команды извлекают его как `State<'_, Arc<T>>`, код без контекста команды — через `app.state::<Arc<T>>()` или `try_state`, когда состояния может ещё не быть. Жёсткие зависимости передаются через конструктор, поздние и необязательные ищутся через состояние.

```rust
// src-tauri/src/lib.rs
let model_manager =
    Arc::new(ModelManager::new(app_handle).expect("Failed to initialize model manager"));
let transcription_manager = Arc::new(
    TranscriptionManager::new(app_handle, model_manager.clone())
        .expect("Failed to initialize transcription manager"),
);
app_handle.manage(transcription_manager.clone());
```

Неочевидное: конструкторы делают реальную работу (создают каталоги, сканируют модели, выполняют миграции, могут открыть микрофон в режиме «всегда включён»), а ошибка любого из них — `expect`, то есть падение приложения. `TranscriptionManager` и `AudioRecordingManager` сами по себе `Clone`, все их поля — `Arc`, так что `Arc<T>` в состоянии — это `Arc` вокруг дешёвого хэндла.

Перенос: регистрировать всё как `Arc<T>` и всегда извлекать `State<'_, Arc<T>>`; смешение `State<T>` и `State<Arc<T>>` ломается только в рантайме (раздел 4).

### 2.3 Отложенная инициализация под разрешения ОС

Где: `src-tauri/src/commands/mod.rs:142` (`initialize_enigo`), `src-tauri/src/commands/mod.rs:180` (`initialize_shortcuts`), `src-tauri/src/commands/mod.rs:173` (`ShortcutsInitialized`).

Эмуляция клавиатуры и регистрация глобальных шорткатов при старте бэкенда не запускаются: на macOS обе вызывают системные запросы разрешений, которые пользователь увидел бы до онбординга. Фронтенд проверяет разрешения через `tauri-plugin-macos-permissions` и после этого вызывает две команды. Команды идемпотентны: `initialize_enigo` проверяет `try_state`, а `initialize_shortcuts` кладёт в состояние маркерный тип `ShortcutsInitialized` и при повторном вызове ничего не делает.

Перенос: всё, что трогает системные разрешения, выносить в идемпотентные команды инициализации, которые UI вызывает после онбординга.

### 2.4 Соглашения команд

Где: `src-tauri/src/lib.rs:650` (`collect_commands!`), `src-tauri/src/commands/models.rs:41` (`download_model`), `src-tauri/src/commands/models.rs:95` (`switch_active_model`), `src-tauri/src/commands/history.rs:64` (`retry_history_entry_transcription`).

Команды разложены по модулям `commands/{mod,audio,models,transcription,history}.rs`; около 60 сеттеров настроек лежат в `shortcut/mod.rs`. Типичная сигнатура — синхронная или `async fn` с `AppHandle` и/или `State<'_, Arc<Manager>>` и типизированными аргументами, результат `Result<T, String>`. Менеджеры возвращают `anyhow`, на границе ошибка превращается в строку через `map_err(|e| e.to_string())` или `format!`. Блокирующая работа внутри асинхронных команд уходит в `spawn_blocking`. Логика, нужная и команде, и трею, вынесена в обычную функцию: `switch_active_model` вызывают и фронтенд, и меню трея.

Когда ошибку надо показать пользователю уже после возврата команды, команда и логирует её, и отправляет событие: `download_model` при сбое шлёт `model-download-failed` (`src-tauri/src/commands/models.rs:55`, `model-download-failed`) и возвращает `Err`.

Перенос: единая сигнатура `State<'_, Arc<T>>` + `Result<T, String>`, общая логика в обычных функциях, событие для ошибок, которые должны пережить вызов.

### 2.5 Координатор транскрипции: чистый автомат на одном потоке

Где: `src-tauri/src/transcription_coordinator.rs:219` (`CoordinatorState`), `src-tauri/src/transcription_coordinator.rs:250` (`on_input`), `src-tauri/src/transcription_coordinator.rs:542` (`new`), `src-tauri/src/transcription_coordinator.rs:671` (`run_effect`), `src-tauri/src/settings.rs:162` (`ShortcutActivation`).

Координатор — лучший образец в кодовой базе. `new` запускает один поток, который владеет `CoordinatorState` и читает канал команд `Input`, `Cancel`, `ProcessingFinished`. Обработчики (`on_input`, `on_grace_expired`, `on_cancel`, `on_processing_finished`) — чистые функции: они меняют состояние и возвращают необязательный эффект `Start` или `Stop`, но не трогают приложение. Выполняет эффекты только `run_effect` через реестр действий. Текущее время передаётся аргументом, поэтому тесты гоняют настоящие переходы на синтетических часах. Тело потока завёрнуто в `catch_unwind`.

```rust
// src-tauri/src/transcription_coordinator.rs
let cmd = if let Some(deadline) = state.grace_deadline() {
    match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(cmd) => cmd,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            if let Some(effect) = state.on_grace_expired() { run_effect(&app, &mut state, effect); }
            continue;
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => break,
    }
} else { match rx.recv() { Ok(cmd) => cmd, Err(_) => break } };
```

Три режима активации проходят через один и тот же автомат и различаются только тем, что завершает запись. В push-to-talk запись останавливает отпускание клавиши. В toggle отпускание игнорируется, останавливает следующее нажатие. В hold-or-toggle (режим по умолчанию, порог 300 мс) отпускание после долгого удержания останавливает запись, а короткое нажатие фиксирует её до следующего нажатия.

Гонки закрыты явно. Нажатия физических клавиш проходят антидребезг 30 мс. Отпускание откладывается на 50 мс, чтобы автоповтор X11 успел его отменить. Нажатие во время обработки предыдущей записи запоминается, а второе такое нажатие отменяет первое, чтобы сохранить чётность переключения. Отмена во время обработки не сбрасывает состояние, а ждёт `ProcessingFinished`. Старт оптимистичен и откатывается, если микрофон на самом деле не открылся (`on_start_result`). Внешние триггеры (сигнал, флаги CLI через single-instance) всегда работают как toggle и антидребезгу не подлежат.

Перенос: сводить все входы жизненного цикла в один канал к чистому редьюсеру, возвращающему эффекты, и передавать время параметром — такой автомат тестируется без `AppHandle`.

### 2.6 Реестр действий и гард завершения

Где: `src-tauri/src/actions.rs:930` (`ACTION_MAP`), `src-tauri/src/actions.rs:52` (`ShortcutAction`), `src-tauri/src/actions.rs:37` (`FinishGuard`), `src-tauri/src/shortcut/handler.rs:29` (`handle_shortcut_event`), `src-tauri/src/utils.rs:86` (`cancel_current_operation`).

`ACTION_MAP` — ленивый `HashMap<String, Arc<dyn ShortcutAction>>` с ключами-идентификаторами привязок: `transcribe`, `transcribe_with_post_process` (та же структура с флагом), `cancel`, `test`. Оба бэкенда шорткатов вызывают один `handle_shortcut_event(binding_id, hotkey, is_pressed)`: привязки транскрипции уходят в координатор, `cancel` срабатывает только на нажатие и только во время записи, остальные сопоставляют нажатие со `start()`, отпускание со `stop()`.

`stop()` запускает асинхронную задачу, первая строка которой создаёт `FinishGuard`. Его `Drop` выгружает модель, если так настроено, сообщает координатору о конце обработки и освобождает память, и срабатывает на любом раннем возврате, включая ошибки. Без этого гарда любой забытый путь ошибки оставил бы координатор в состоянии «обработка» навсегда.

```rust
// src-tauri/src/actions.rs
impl Drop for FinishGuard {
    fn drop(&mut self) {
        self.1.maybe_unload_immediately("transcription session");
        if let Some(c) = self.0.try_state::<TranscriptionCoordinator>() {
            c.notify_processing_finished();
        }
        crate::memory::trim_freed_memory();
    }
}
```

Отмена собрана в одну функцию `cancel_current_operation`: остановить запись, отменить поток распознавания, сбросить трей и оверлей, уведомить координатор.

Перенос: если автомат ждёт завершения асинхронной задачи, сигнал о завершении отправлять из `Drop`-гарда.

### 2.7 Тяжёлый ресурс: ленивая загрузка, выгрузка по простою, перезагрузка при следующем использовании

Где: `src-tauri/src/managers/transcription.rs:307` (`thread::spawn`), `src-tauri/src/managers/transcription.rs:401` (`try_start_loading`), `src-tauri/src/managers/transcription.rs:744` (`initiate_model_load`), `src-tauri/src/managers/transcription.rs:393` (`reload_model_on_next_use`), `src-tauri/src/managers/transcription.rs:1176` (`transcribe`), `src-tauri/src/managers/transcription.rs:1302` (`catch_unwind`).

Модель распознавания весит гигабайты, и работа с ней построена на четырёх приёмах. Загрузка идёт в одном экземпляре: флаг `is_loading` под мьютексом плюс `Condvar`, `transcribe()` ждёт окончания загрузки, а RAII-гард `LoadingGuard` снимает флаг и будит ожидающих, восстанавливаясь после отравленного мьютекса, чтобы его `Drop` не паниковал. Загрузка начинается одновременно с открытием микрофона, а не после окончания записи. Фоновый поток раз в 10 секунд перечитывает таймаут простоя и выгружает модель, если с последней активности прошло больше; во время записи таймер обновляется. Смена ускорителя (CPU/GPU) не перезагружает модель немедленно, а ставит атомарный флаг «перезагрузить при следующем использовании», так что идущая транскрипция доделывается на старом движке.

Во время распознавания движок вынимается из `Mutex<Option<LoadedEngine>>`, мьютекс отпускается, и инференс идёт внутри `catch_unwind(AssertUnwindSafe(..))`. После успеха или обычной ошибки движок возвращается, только если идентификатор текущей модели не поменялся; паника приводит к выгрузке движка и событию `model-state-changed` с ошибкой вместо отравленного мьютекса. Каждое изменение состояния модели (`loading_started`, `loading_failed`, `loading_completed`, `unloaded`) уходит событием `model-state-changed`.

Перенос: для тяжёлого нативного ресурса — загрузка в одном экземпляре, таймер простоя, флаг «устарел, перезагрузить потом» и никакой блокировки на время долгого нативного вызова.

### 2.8 Счётчики поколений и атомики на горячих путях

Где: `src-tauri/src/managers/audio.rs:374` (`AudioRecordingManager`), `src-tauri/src/managers/audio.rs:975` (`cancel_generation`), `src-tauri/src/managers/audio.rs:1060` (`is_recording`), `src-tauri/src/managers/transcription.rs:121` (`StreamRouter`).

Вместо флагов отмены, которые надо не забыть сбросить, менеджер записи использует три монотонных счётчика `AtomicU64`. `cancel_generation` запоминается задачей остановки и перепроверяется на каждой точке `await`: если счётчик вырос, операцию отменили. `capture_generation` отбрасывает запоздавшие события «микрофон готов» и звуковой сигнал старта от предыдущей записи. `close_generation` защищает ленивое закрытие микрофона через 30 секунд простоя: поток закрытия ничего не делает, если за это время счётчик увеличился. Оверлей и трей используют тот же приём (раздел 2.14).

`is_recording()` читает зеркало состояния в `AtomicBool`, которое обновляется вместе с основным состоянием, поэтому опрос из UI не ждёт мьютекса, пока CoreAudio медленно открывает устройство. `StreamRouter::feed` проверяет атомик до захвата мьютекса, а аудиокадры и команда финализации идут по одному каналу, так что порядок FIFO гарантирует, что все кадры обработаны до финализации.

Перенос: поколения вместо флагов отмены и атомарное зеркало для любого состояния, которое часто опрашивает UI.

### 2.9 RAII-гарды для загрузок и пересканирования

Где: `src-tauri/src/managers/model.rs:498` (`DownloadCleanup`), `src-tauri/src/managers/model.rs:485` (`RescanGuard`), `src-tauri/src/managers/model.rs:1204` (`try_start_rescan`), `src-tauri/src/managers/model.rs:2639` (`cancel_download`), `src-tauri/src/managers/model/download.rs:45` (`HttpDownloadEvent`).

`DownloadCleanup` снимает признак загрузки и удаляет токен отмены на каждом пути выхода через `?`, если его явно не разоружили после успеха. Пересканирование каталога моделей идёт в одном экземпляре через `AtomicBool::swap`, берёт снимок реестра, сканирует диск без удержания блокировки и затем добавляет только новые идентификаторы. Отмена загрузки — `CancellationToken` на идентификатор модели. Загрузчик докачивает через `Range`, имеет таймаут простоя 60 секунд и подключения 15 секунд, проверяет SHA-256 и не зависит от Tauri: вместо `AppHandle` он получает колбэк событий, поэтому тестируется против локального сокет-сервера. События прогресса прорежены до одного в 100 мс.

Перенос: каждый флаг «идёт операция» снимать гардом; дисковый ввод-вывод делать по снимку, а не под блокировкой; транспорт отвязывать от Tauri через колбэк событий.

### 2.10 Настройки: значения по умолчанию, спасение полей, миграции

Где: `src-tauri/src/settings.rs:363-364` (`serde(default)`), `src-tauri/src/settings.rs:1014` (`get_settings`), `src-tauri/src/settings.rs:1067` (`salvage_settings`), `src-tauri/src/settings.rs:1100` (`apply_settings_migrations`), `src-tauri/src/settings.rs:1209` (`write_settings`), `src-tauri/src/settings.rs:523` (`CURRENT_SETTINGS_SCHEMA_VERSION`).

Все настройки — одна структура `AppSettings` с `#[serde(default)]` на уровне контейнера и функциями по умолчанию на отдельных полях, хранящаяся как один JSON-объект под ключом `settings` в `settings_store.json` через `tauri-plugin-store`. Платформенные значения задаются через `cfg`: шорткат `option+space` на macOS и `ctrl+space` на остальных, стиль оверлея по умолчанию выключен на Linux.

Каждый вызов `get_settings` читает значение и пытается десериализовать его целиком. Если не вышло, `salvage_settings` начинает с сериализованных значений по умолчанию и накладывает сохранённые ключи по одному, после каждой вставки проверяя, что объект по-прежнему десериализуется; ключ, который ломает десериализацию, возвращается к значению по умолчанию, а в лог пишется только его имя, потому что значения могут быть секретами. Одно испорченное поле не сбрасывает остальные настройки.

```rust
// src-tauri/src/settings.rs
for (key, value) in stored_map {
    let previous = merged.as_object_mut().expect("merged settings stay an object")
        .insert(key.clone(), value.clone());
    if serde_json::from_value::<AppSettings>(merged.clone()).is_err() {
        warn!("Dropping invalid settings field '{key}', keeping its default");
        let map = merged.as_object_mut().expect("merged settings stay an object");
        match previous { Some(previous) => map.insert(key.clone(), previous), None => map.remove(key) };
    }
}
```

Затем идут идемпотентные миграции. Они смотрят на сырой JSON, а не на десериализованную структуру, потому что в структуре отсутствующий ключ уже заменён значением по умолчанию и неотличим от явно сохранённого. После миграций в привязки добавляются новые шорткаты, появившиеся в новых версиях, и настройки записываются обратно, только если что-то изменилось. Совместимость форматов обеспечивают собственные десериализаторы: `LogLevel` принимает и старые числа, и новые строки, у перечислений есть `#[serde(alias)]`. Для секретов есть `SecretMap`, у которого `Debug` скрывает значения.

Запись — чтение, изменение, `write_settings` в каждом сеттере; сохранение на диск делает плагин с отложенной на 100 мс автозаписью и записью при выходе. Кэша настроек в памяти поверх плагина нет, потребители перечитывают `get_settings`, когда им нужно значение. Флаги командной строки и переменные окружения (`--debug`, `HANDY_DISABLE_UPDATER`) применяются к локальной копии и в файл не попадают.

Неочевидное: спасение работает на уровне полей, но не файла. Плагин игнорирует ошибку загрузки файла, поэтому непарсящийся `settings_store.json` выглядит как пустое хранилище, и `get_settings` записывает поверх него значения по умолчанию (раздел 4).

Перенос: `serde(default)` на контейнере с единственным `Default`, спасение по ключам, миграции по сырому JSON плюс номер версии схемы, запись только при изменении, переопределения времени выполнения вне сохраняемой структуры. Дополнительно — делать резервную копию файла, который не удалось разобрать.

### 2.11 История в SQLite с упорядоченными миграциями

Где: `src-tauri/src/managers/history.rs:20` (`MIGRATIONS`), `src-tauri/src/managers/history.rs:99` (`init_database`), `src-tauri/src/managers/history.rs:142` (`migrate_from_tauri_plugin_sql`).

Схема ведётся `rusqlite_migration` по `user_version`, в отладочной сборке миграции дополнительно валидируются. Для пользователей старых версий, у которых схему вёл `tauri-plugin-sql`, состояние таблицы `_sqlx_migrations` однократно переносится в `user_version`. Менеджер не держит соединение: каждый метод открывает новое `Connection`. Методы объявлены `async fn`, но вызывают синхронный rusqlite без `spawn_blocking`, что блокирует поток асинхронного рантайма на время запроса.

Перенос: упорядоченный список миграций и мост со старого механизма миграций; для асинхронных методов — `spawn_blocking`.

### 2.12 Логирование

Где: `src-tauri/src/lib.rs:53` (`FILE_LOG_LEVEL`), `src-tauri/src/lib.rs:61` (`WEBVIEW_LOG_STREAMING`), `src-tauri/src/utils.rs:17` (`redact_text`).

Глобальный уровень — `Trace`, а три цели фильтруются отдельно. Консоль уважает `RUST_LOG` и по умолчанию показывает Info; в headless-режиме она пишет в stderr, чтобы stdout оставался машиночитаемым. Файл `handy.log` ротируется на 500 КБ с одной старой копией, и его уровень задаётся атомиком, который меняется из настроек без пересборки логгера. Трансляция логов в webview включается только в debug-режиме. `--debug` поднимает уровень до Trace на одну сессию. Тексты транскрипций в релизных логах маскируются.

Перенос: уровни целей как атомики, читаемые внутри фильтров, и трансляция логов в UI только под debug-флагом.

### 2.13 Глобальные шорткаты

Где: `src-tauri/src/shortcut/mod.rs:33` (`init_shortcuts`), `src-tauri/src/settings.rs:203` (`KeyboardImplementation`), `src-tauri/src/shortcut/mod.rs:113` (`change_binding`), `src-tauri/src/shortcut/mod.rs:235` (`suspend_all_shortcuts`), `src-tauri/src/shortcut/handy_keys.rs:110` (`manager_thread`), `src-tauri/src/shortcut/mod.rs:500` (`initialize_handy_keys_with_rollback`).

Бэкендов два, и выбирает их сохранённая настройка: плагин Tauri `global-shortcut` (по умолчанию на Linux) и крейт `handy-keys` (по умолчанию на macOS и Windows). Каждый вызов регистрации перечитывает настройку и диспетчеризует в нужную реализацию. Если `handy-keys` не стартовал, в настройки записывается `Tauri`, чтобы следующий запуск не повторял сбой. При переключении бэкенда на лету все привязки снимаются со старого и проверяются на новом; неподходящие сбрасываются к значениям по умолчанию, их идентификаторы возвращаются в UI, и отправляется `settings-changed`.

Смена привязки — транзакция: снять старую, проверить новую, зарегистрировать, сохранить; при ошибке проверки или регистрации старая привязка восстанавливается (`restore_registration`). Валидаторы у бэкендов разные: плагин Tauri отвергает `fn` и комбинации из одних модификаторов, `handy-keys` принимает всё, что разбирается как `Hotkey`. Имена клавиш платформенные: на macOS `option`/`command`, на остальных `alt`/`super`.

Пока пользователь записывает новую комбинацию, все привязки, кроме отмены, приостанавливаются, а на любом выходе из записи — сохранении или отмене — возобновляются. С бэкендом Tauri клавиши ловит DOM в webview; с `handy-keys` бэкенд открывает отдельный слушатель и шлёт события `handy-keys-event`.

`HotkeyManager` из `handy-keys` создаётся внутри выделенного потока и никогда его не покидает. Остальной код отправляет ему команды `Register`/`Unregister` по каналу вместе с каналом ответа и ждёт результат; цикл потока вычерпывает события клавиш и ждёт команды до 10 мс. `Drop` отправляет `Shutdown` и дожидается потока.

Шорткат отмены (по умолчанию Escape) регистрируется только на время записи и снимается после неё, чтобы не перехватывать Escape во всей системе. На Linux динамическая регистрация была нестабильной, и там отмена шорткатом отключена.

Перенос: бэкенд горячих клавиш за перечислением с сохраняемым безопасным откатом; смена привязки как транзакция с восстановлением; приостановка своих шорткатов на время записи нового; не-`Send` библиотеку — в поток-актор с каналами запрос/ответ; клавиши, нужные в одном состоянии, регистрировать только в этом состоянии.

### 2.14 Трей: желаемое состояние и один применяющий

Где: `src-tauri/src/tray.rs:245` (`sync_tray_with`), `src-tauri/src/tray.rs:70` (`TrayDesired`), `src-tauri/src/tray.rs:312` (`compute_desired`), `src-tauri/src/tray.rs:354` (`apply_on_main`), `src-tauri/src/tray.rs:458` (`build_menu`), `src-tauri/src/tray.rs:191` (`get_icon_path`).

Код, меняющий состояние, не трогает трей напрямую. Он записывает намерение и получает порядковый номер в одной критической секции, затем на своём потоке вычисляет снимок желаемого состояния (настройки, список моделей, тема, иконка), чтобы главный поток никогда не брал блокировки менеджеров. На главный поток планируется одно применение; запросы, пришедшие до его выполнения, сливаются в него, а устаревшие по номеру отбрасываются. Применяющий сравнивает желаемое с последним успешно применённым и меняет только отличия, частичные сбои повторяются при следующей синхронизации. Схема появилась из-за бага macOS, при котором значок трея пропадал, и гонок перестроения меню из нескольких потоков.

Иконка выбирается по теме и состоянию (ожидание, запись, распознавание) плюс значок-предупреждение о Secure Input. На Windows тема берётся из реестра (`SystemUsesLightTheme`), потому что панель задач может иметь другую тему, чем приложения. Меню во время работы показывает «Отмена», в простое — подменю моделей на `CheckMenuItem` с идентификаторами `model_select:<id>` и «Выгрузить модель». Клики обрабатывает один `on_menu_event` по строковому идентификатору; смена модели идёт в отдельном потоке. Если обновления принудительно выключены переменной окружения, пункт проверки обновлений удаляется из меню, а не делается неактивным.

Перенос: обновлять трей из сохранённого снимка желаемого состояния через одного применяющего на главном потоке, а не из каждого места, где меняется состояние.

### 2.15 Жизненный цикл окна у трей-приложения

Где: `src-tauri/src/lib.rs:143` (`apply_startup_activation_policy`), `src-tauri/src/lib.rs:97` (`show_main_window`), `src-tauri/src/lib.rs:1056` (`CloseRequested`), `src-tauri/src/tray.rs:621` (`recreate_tray_icon`).

Главное окно создаётся скрытым и показывается, если не задан скрытый старт или если трея нет. Закрытие окна вызывает `prevent_close` и прячет его; на macOS при видимом трее приложение переключается в политику `Accessory` и убирает значок из Dock. `show_main_window` разворачивает, показывает, фокусирует окно и возвращает политику `Regular`. При скрытом старте политика `Accessory` выставляется между `build()` и `run()`, потому что понижение на лету оставляло значок в Dock на macOS 26. Повторный запуск приложения или `Reopen` пересоздают значок трея, возвращая пропавший элемент строки меню. Без трея (настройка или `--no-tray`) окно показывается всегда, чтобы пользователь не остался без способа вернуться в приложение.

Перенос: одно правило «трей доступен» для скрытого старта и закрытия в трей; политику активации выбирать до запуска цикла событий; повторный запуск как способ починить пропавший значок.

### 2.16 Оверлей записи

Где: `src-tauri/src/overlay.rs:452` (`create_recording_overlay`), `src-tauri/src/overlay.rs:27` (`RecordingOverlayPanel`), `src-tauri/src/overlay.rs:121` (`init_gtk_layer_shell`), `src-tauri/src/overlay.rs:246` (`calculate_overlay_position`), `src-tauri/src/overlay.rs:686` (`OVERLAY_SHOW_GENERATION`), `src-tauri/src/overlay.rs:689` (`hide_recording_overlay`), `src-tauri/src/overlay.rs:730` (`emit_levels`).

Окно оверлея создаётся один раз при старте, скрытым, и дальше только перемещается и показывается. Тип окна свой на каждой платформе. На macOS это панель `tauri-nspanel` уровня `Status` без активации, видимая на всех рабочих столах и поверх полноэкранных приложений, которая не может стать ключевым окном. На Windows и Linux — прозрачное окно без рамки поверх всех, без значка в панели задач, `focusable(false)`. На Linux дополнительно используется gtk-layer-shell со слоем `Overlay` и без захвата клавиатуры; `HANDY_NO_GTK_LAYER_SHELL` или отсутствие поддержки возвращают обычное окно. Оверлей не пропускает клики насквозь: у него есть кнопка отмены.

Позиция считается на мониторе под курсором, по центру по горизонтали, сверху или снизу; снизу на macOS используется рабочая область, чтобы не перекрывать Dock. Координаты задаются логическими, потому что физические пересчитывались бы с масштабом монитора, на котором окно находится сейчас, а не целевого. Windows ставит окно через `SetWindowPos` в физических пикселях целевого монитора с учётом масштаба текста и повторяет это после показа, потому что `WM_DPICHANGED` сбивает первое размещение.

Показ проверяет стиль оверлея вне главного потока, затем переходит на главный поток для работы с геометрией (обращение из другого потока портило общее соединение X11), ставит размер под состояние, показывает окно и отправляет `show-overlay`. Скрытие отправляет `hide-overlay` для анимации и прячет окно через 300 мс, если за это время счётчик показов не вырос.

```rust
// src-tauri/src/overlay.rs
let scheduled_at = OVERLAY_SHOW_GENERATION.load(Ordering::SeqCst);
let _ = overlay_window.emit("hide-overlay", ());
let window_clone = overlay_window.clone();
std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_millis(300));
    if OVERLAY_SHOW_GENERATION.load(Ordering::SeqCst) != scheduled_at {
        return;
    }
    let _ = window_clone.hide();
});
```

Уровни микрофона проходят через кэшированный атомарный флаг «оверлей включён», прореживаются до одного события в 33 мс и отправляются через `emit_to("recording_overlay", "mic-level")` только окну оверлея. Широковещательная рассылка давала лишние вызовы `eval_script` и рост памяти WebKit.

Перенос: HUD-окно создавать один раз, с неактивирующимся нативным типом окна на каждой ОС; геометрию считать в координатах целевого монитора; отложенное скрытие защищать счётчиком показов; высокочастотные события слать одному окну с прореживанием.

### 2.17 Вставка текста через буфер обмена

Где: `src-tauri/src/clipboard.rs:774` (`paste`), `src-tauri/src/clipboard.rs:55` (`paste_via_clipboard`), `src-tauri/src/input.rs:77` (`resolve_command_v_keycode`), `src-tauri/src/paste_tx/mod.rs:194` (`try_reliable_paste`).

Методов вставки шесть: `CtrlV`, `CtrlShiftV`, `ShiftInsert`, прямой набор через Enigo, внешний скрипт и «не вставлять»; по умолчанию прямой набор на Linux и `CtrlV` на остальных. Путь через буфер: сохранить текущее содержимое (текст, а если текста нет — изображение), записать транскрипт, подождать 60 мс, отправить сочетание клавиш, подождать ещё 60 мс и восстановить исходное содержимое, в том числе если отправка клавиш не удалась. На macOS код клавиши «V» вычисляется для текущей раскладки через `UCKeyTranslate`, поэтому вставка работает в Dvorak и нелатинских раскладках; вызов идёт на главном потоке, потому что API источников ввода macOS работают только там. На Linux перебирается цепочка инструментов: на Wayland wtype (кроме KDE и GNOME), dotool, ydotool, на X11 xdotool и ydotool, в конце Enigo.

Экспериментальная «надёжная вставка» (выключена по умолчанию, macOS и Windows) кладёт в буфер не текст, а обещание данных: на macOS через `declareTypes:owner:`, на Windows через `SetClipboardData(CF_UNICODETEXT, NULL)` от окна-владельца с собственным циклом сообщений. Когда целевое приложение читает буфер, система вызывает провайдер, и Handy знает, что вставка произошла. Восстановление ждёт 200 мс тишины после последнего чтения, но не больше 8 секунд, и выполняется, только если буфер всё ещё принадлежит Handy. Транскрипт помечается так, чтобы менеджеры буфера и история буфера Windows его не сохраняли.

Перенос: всегда восстанавливать буфер пользователя, включая пути ошибок; вычислять код клавиши вставки для активной раскладки; при возможности использовать отложенный рендеринг буфера вместо фиксированных задержек.

### 2.18 Secure Input на macOS

Где: `src-tauri/src/secure_input.rs:294` (`start_monitor`), `src-tauri/src/secure_input.rs:459` (`reconcile_fallback`).

Когда какое-то приложение (обычно менеджер паролей или поле пароля) включает Secure Input, бэкенд `handy-keys`, работающий через перехват событий, перестаёт получать нажатия клавиш, хотя изменения модификаторов продолжают приходить. Поток раз в секунду опрашивает `IsSecureEventInputEnabled()` и через 3 секунды считает режим застрявшим. Тогда для затронутых привязок регистрируются теневые копии через плагин Tauri (Carbon), который Secure Input не блокирует; привязки из одних модификаторов тени не требуют, `fn` покрыть нельзя, а различие левого и правого модификатора теряется, о чём сообщается как о деградации. Пользователь видит предупреждение в UI и значок в трее, только если какая-то привязка действительно пострадала. Приложение, удерживающее Secure Input, определяется разбором `ioreg`.

Перенос: обнаруживать состояния ОС, молча ломающие бэкенд горячих клавиш, закрывать затронутые привязки вторым бэкендом и сообщать пользователю, только если что-то действительно потеряно.

### 2.19 Звуки и приглушение системного звука

Где: `src-tauri/src/audio_feedback.rs:97` (`play_audio_file`), `src-tauri/src/managers/audio.rs:586` (`apply_mute`), `src-tauri/src/managers/audio.rs:611` (`remove_mute`).

Звук старта играет, только когда микрофон отдал первые сэмплы и эта запись всё ещё текущая, а не в момент открытия потока; приглушение системного звука включается уже после сигнала. При остановке приглушение снимается до звука остановки, чтобы его было слышно. Приглушение на Windows идёт через `IAudioEndpointVolume::SetMute`, на Linux через цепочку `wpctl` → `pactl` → `amixer`, на macOS через `osascript`; предыдущее состояние сохраняется, и если система уже была приглушена, она такой и останется.

Перенос: привязывать звук готовности к реальной готовности захвата и всегда сохранять пользовательское состояние перед изменением.

### 2.20 Автозапуск

Где: `src-tauri/src/autostart.rs:21` (`apply_autostart`).

Автозапуск применяется при каждом запуске и при изменении настройки. На macOS 13+ используется `SMAppService.mainAppService` (наличие класса проверяется в рантайме), и заодно удаляется старый LaunchAgent из `~/Library/LaunchAgents`, чтобы приложение не стартовало дважды. LaunchAgent плагина отображается в системных настройках под именем разработчика, а не приложения, что и стало причиной перехода. На остальных платформах работает `tauri-plugin-autostart`.

### 2.21 Удалённое управление и headless-режим

Где: `src-tauri/src/lib.rs:852` (`tauri_plugin_single_instance`), `src-tauri/src/signal_handle.rs:35` (`setup_signal_handler`), `src-tauri/src/cli.rs:6` (`CliArgs`), `src-tauri/src/lib.rs:434` (`run_headless_transcription`).

Второй запуск приложения передаёт свои аргументы работающему экземпляру через `tauri_plugin_single_instance`. Колбэк сравнивает строки `--toggle-transcription`, `--toggle-post-process`, `--cancel` и отправляет соответствующую команду координатору; любой другой повторный запуск просто показывает окно. На Unix поток сигналов переводит SIGUSR2 в «транскрибировать», SIGUSR1 — в «с постобработкой», но только на macOS, потому что на Linux SIGUSR1 использует сборщик мусора WebKitGTK. По чтению кода, флаги удалённого управления из разобранной структуры clap нигде не читаются: они работают только как аргументы, пересланные уже запущенному экземпляру.

Флаги `--transcribe-file`, `--list-devices`, `--list-models` включают headless-режим: single-instance пропускается, в `setup` строятся только менеджеры моделей и транскрипции, работа идёт в потоке под `catch_unwind`, логи уходят в stderr, коды выхода — 0 при успехе, 1 при ошибке выполнения, 2 при неверном вводе.

Перенос: все внешние триггеры сводить в одну точку входа с пометкой источника; headless-путь на тех же менеджерах даёт дешёвый бенчмарк и дымовой тест в CI.

## 3. Приёмы UX

### 3.1 Онбординг из двух шагов и завершение по первому успеху

Где: `src/App.tsx:225` (`checkOnboardingStatus`), `src-tauri/src/commands/models.rs:124` (`onboarding_completed`).

Первый запуск — два шага: разрешения, затем модель. Кнопки «Готово» нет: фронтенд вообще не записывает признак завершения онбординга. Его ставит бэкенд как побочный эффект первого успешного выбора активной модели. Для пользователей, у которых модель уже была выбрана до появления этого признака, миграция настроек отмечает онбординг пройденным. Вернувшийся пользователь попадает только на тот шаг, условие которого нарушено, — например, если разрешение отозвали.

Перенос: считать онбординг пройденным по первому реальному результату, а не по нажатию «Завершить».

### 3.2 Карточки разрешений с опросом

Где: `src/components/onboarding/AccessibilityOnboarding.tsx:21` (`PermissionStatus`), `src/components/onboarding/AccessibilityOnboarding.tsx:174` (`startPolling`), `src/components/onboarding/AccessibilityOnboarding.tsx:49` (`MAX_POLLING_ERRORS`).

Каждое разрешение — карточка: круглая иконка, заголовок, одна строка о том, зачем разрешение нужно, и одно из трёх состояний: кнопка «Дать разрешение», спиннер с «Ожидание…» или зелёная галочка «Выдано». Статусов четыре: `checking`, `needed`, `waiting`, `granted`. Нажатие вызывает системный запрос (на Windows открывает раздел конфиденциальности в настройках) и запускает опрос раз в секунду. Когда всё выдано, список устройств обновляется, на 300 мс показывается «Всё готово!», и онбординг идёт дальше. Три ошибки опроса подряд останавливают цикл и показывают тост. Баннер разрешений внутри приложения (`src/components/AccessibilityPermissions.tsx:36`) опроса не делает: там надо нажать ещё раз, чтобы перепроверить.

Перенос: для системных диалогов вне приложения — опрос после действия пользователя и явное состояние «ждём» на каждую карточку.

### 3.3 Кураторский выбор первой модели

Где: `src/components/onboarding/Onboarding.tsx:43` (`useMemo`).

Устаревшие модели скрыты. Первые две модели с флагом `is_recommended` в порядке ранга из бэкенда показываются крупными карточками `variant="featured"`, остальные рекомендованные — обычными, всё прочее спрятано за «Показать все {{total}} моделей» со стрелкой, которая поворачивается при раскрытии. Клик по карточке запускает загрузку и приглушает остальные карточки. Эффект ждёт, пока модель загружена, проверена и распакована, затем сам выбирает её и выходит из онбординга; ref защищает от двойного срабатывания. Уже скачанные модели показываются отдельным разделом и выбираются без загрузки.

Перенос: две сильные рекомендации сверху, длинный хвост за раскрытием, один клик и для загрузки, и для активации.

### 3.4 Карточка модели как функция одного статуса

Где: `src/components/onboarding/ModelCard.tsx:56` (`ModelCardStatus`), `src/components/onboarding/ModelCard.tsx:99-101` (`isClickable`), `src/components/settings/models/ModelsSettings.tsx:98` (`getModelStatus`).

Карточка получает один проп статуса из семи значений: `downloadable`, `downloading`, `verifying`, `extracting`, `switching`, `active`, `available`. Сверху — название, бейджи (рекомендована, активна, пользовательская, устаревшая, переключается), описание и две полоски по 64 px: точность и скорость. Внизу — чипы «иконка + текст» (число языков или «только X», перевод, потоковый режим) и размер справа с иконкой загрузки или диска. Во время загрузки — тонкая полоса, процент, МБ/с и кнопка отмены в стиле `danger-ghost`; во время проверки и распаковки — пульсирующая полоса на всю ширину. Активная модель намеренно некликабельна, потому что повторный выбор только перезагрузил бы её без пользы. Состояния ошибки у карточки нет: сбой показывается тостом, и карточка возвращается в `downloadable`.

Перенос: одна карточка, управляемая одним статусом, служит и онбордингу, и библиотеке моделей.

### 3.5 Статус-точка в футере, она же переключатель

Где: `src/components/model-selector/ModelSelector.tsx:156` (`getModelDisplayText`), `src/components/model-selector/ModelStatusButton.tsx:28` (`getStatusColor`).

Слева в футере — точка 8 px и название текущей модели, обрезанное по `max-w-28`. Цвет точки кодирует состояние: зелёный — готова, пульсирующий жёлтый — загружается в память, пульсирующий розовый — скачивается, пульсирующий оранжевый — проверяется или распаковывается, красный — ошибка или модели нет, серый — выгружена. Прогресс скачивания имеет приоритет над состоянием загрузки. Во время переключения подпись меняется сразу, до окончания переключения. Клик открывает список вверх со скачанными моделями и меткой «Активна». Через 500 мс после окончания скачивания новая модель выбирается автоматически, если не идёт запись.

Перенос: чип состояния в футере, который одновременно переключатель, показывает состояние системы всё время и почти не занимает места.

### 3.6 Прогресс скачивания со сглаженной скоростью

Где: `src/stores/modelStore.ts:162` (`downloadModel`), `src/stores/modelStore.ts:302-308` (`speed`).

`downloadModel` сразу записывает прогресс 0 %, поэтому полоса появляется в момент клика, а не после первого события. Скорость пересчитывается не чаще раза в 0,5 секунды как экспоненциальное среднее с весами 0,8 для старого и 0,2 для нового значения, поэтому цифра МБ/с читается, а не мелькает. Одна загрузка показывается полосой со скоростью, несколько — мини-полосами по 12 px и подписью «N загружается…».

### 3.7 Подтверждение только для дорогих действий

Где: `src/components/settings/models/ModelsSettings.tsx:151` (`ask`).

Единственное подтверждение во всём фронтенде — удаление модели, через нативный диалог Tauri `ask` с `kind: "warning"`. Текст зависит от ситуации: для активной модели сказано, что распознавание перестанет работать, для остальных — что модель придётся скачивать заново. Удаление записи истории оптимистично и без подтверждения, сброс настроек к значению по умолчанию — тоже.

Перенос: подтверждать только то, что дорого отменить (повторная загрузка нескольких гигабайт), и называть в тексте конкретное последствие.

### 3.8 Оверлей: «взведён» → живая волна → работа

Где: `src/overlay/RecordingOverlay.tsx:15` (`OverlayState`), `src/overlay/RecordingOverlay.tsx:19` (`WAVE_BARS`), `src/overlay/RecordingOverlay.tsx:97` (`recording-ready`), `src/overlay/RecordingOverlay.tsx:102` (`mic-level`), `src/overlay/RecordingOverlay.css:378` (`.swave.arming`).

По `show-overlay` оверлей синхронно переходит во «взведённое» состояние: приглушённая точка и серый бегущий импульс по девяти столбикам. Только по `recording-ready`, который бэкенд отправляет после первых реальных сэмплов микрофона, точка становится розовой и пульсирующей, а столбики начинают следовать 16 уровням спектра, приходящим примерно 30 раз в секунду. Так нажатие шортката получает мгновенный отклик, но оверлей не изображает звук, которого ещё нет. Уровни сглаживаются на стороне оверлея: `prev * 0.7 + target * 0.3`. В состояниях `transcribing` и `processing` волна заменяется спиннером и подписью. Строка всегда — сетка из трёх колонок (точка, центр, кнопка отмены), поэтому крестик не прыгает при смене состояния; ширина плавно меняется с 172 до 216 px.

В потоковом режиме, когда появляется текст, карточка расширяется со 184 до 392 px, скругление уменьшается с 24 до 16 px за 460 мс, текстовая область раскрывается приёмом `grid-template-rows: 0fr → 1fr` (`src/overlay/RecordingOverlay.css:224`, `.stext`). Текст курсивом, высотой до 64 px, с маской затухания сверху и мигающей розовой кареткой; прокрутка держится у последней строки, пока пользователь не прокрутит вверх. Во время финализации панель остаётся открытой, а каретка исчезает. Размеры в CSS обязаны совпадать с размерами окна в `overlay.rs`, о чём предупреждает комментарий.

Неочевидное, по чтению кода: когда оверлей скрыт, компонент возвращает `null`, поэтому CSS-анимация исчезновения, по-видимому, не успевает отрисоваться, и фактически оверлей убирает отложенное на 300 мс скрытие окна из бэкенда. Функция очистки слушателей событий создаётся, но эффект её не возвращает.

Перенос: отдельное состояние «взведён, сигнала ещё нет»; раскладка с фиксированными колонками, чтобы элементы управления не двигались; рост индикатора только когда есть что показать.

### 3.9 Запись шортката

Где: `src/components/settings/GlobalShortcutInput.tsx:183` (`startRecording`), `src/components/settings/GlobalShortcutInput.tsx:67` (`handleKeyUp`), `src/components/settings/ShortcutInput.tsx:20-30` (`HandyKeysShortcutInput`).

Клик по чипу шортката приостанавливает все глобальные привязки, чип становится розовым с подписью «Нажмите клавиши…». Нажатые клавиши накапливаются (автоповтор игнорируется), чип показывает отформатированную комбинацию. Сохранение происходит, когда отпущены все клавиши: модификаторы сортируются вперёд, комбинация записывается. Клик снаружи возвращает прежний шорткат, ошибка показывает тост и откатывает значение. Рядом с чипом — кнопка сброса. В варианте для `handy-keys` комбинация с обычной клавишей сохраняется при отпускании этой клавиши, а комбинация из одних модификаторов — когда отпущены все модификаторы. Если мешает Secure Input, тост ошибки содержит действие «Как исправить».

Перенос: сохранять комбинацию по полному отпусканию, а не по `keydown`, и приостанавливать свои глобальные шорткаты на время записи.

### 3.10 История транскрипций

Где: `src/components/settings/history/HistorySettings.tsx:40` (`PAGE_SIZE`), `src/components/settings/history/HistorySettings.tsx:111` (`IntersectionObserver`), `src/components/settings/history/HistorySettings.tsx:318` (`handleCopyText`).

Список подгружается страницами по 30 записей через `IntersectionObserver` с курсором «последний идентификатор». Новые транскрипции добавляются сверху в реальном времени через типизированное событие. У строки — дата, четыре иконки (копировать, отметить звёздочкой, распознать заново, удалить), текст курсивом и аудиоплеер. Копирование на 2 секунды меняет иконку на галочку, ошибка копирования показывается тостом. Звёздочка и удаление оптимистичны: звёздочка откатывается при сбое, после удаления список перечитывается. Повторное распознавание вращает свою иконку в обратную сторону и показывает пульсирующее «Распознаётся…» на месте текста. Пустой транскрипт объясняет, что делать: «Распознать не удалось, можно повторить кнопкой повтора». Пустой список приглашает начать запись.

Перенос: подтверждать действия строки на месте, сменой иконки, а не тостом; пустые и неудачные состояния формулировать как подсказку следующего шага.

### 3.11 Ленивый аудиоплеер

Где: `src/components/ui/AudioPlayer.tsx:29` (`AudioPlayerGroup`).

Аудиофайл не загружается до первого нажатия «Играть», после загрузки воспроизведение начинается само. Общий контекст группы ставит на паузу другой играющий плеер. Позиция обновляется через `requestAnimationFrame` с чтением из ref, чтобы цикл не видел устаревшее состояние; перемотка применяется после окончания перетаскивания. На Linux аудио отдаётся как Blob, на остальных — как asset URL.

Перенос: длинные списки медиа загружать по первому воспроизведению и разрешать одновременно только один плеер.

### 3.12 Обновления как строка в футере

Где: `src/components/update-checker/UpdateChecker.tsx:174` (`getUpdateStatusText`).

Проверка обновлений — кликабельный текст в футере без оформления кнопки. Состояния: «Проверить обновления», «Проверка…», «Актуальная версия» (только после ручной проверки и только на 3 секунды), «Доступно обновление» (розовым, клик устанавливает), «Подготовка…», «Загрузка… N %» с полосой, «Установка…», перезапуск. При монтировании проверка идёт молча. Пункт трея открывает окно и запускает ручную проверку. Портативная установка вместо автообновления получает диалог со ссылкой на ручную загрузку.

Перенос: статус обновлений как тихая постоянная строка; «актуальная версия» показывать, только когда пользователь спросил.

### 3.13 Тосты только для ошибок

Где: `src/App.tsx:300` (`Toaster`), `src/App.tsx:143` (`recording-error`).

Один `Toaster` из `sonner` с `theme="system"` и `unstyled: true`, стилизованный своими классами на токенах, смонтирован рядом с активной веткой контента, над гейтом онбординга. Поэтому ошибки видны и во время онбординга, а переход между онбордингом и приложением не теряет показанные тосты. Комментарий в коде объясняет причину: без этого ошибка загрузки модели в онбординге проглатывалась, и мастер просто «моргал». События ошибок бэкенда сопоставляются с переведёнными заголовком и описанием; для ошибки микрофона текст зависит от платформы. Сырое сообщение уходит в лог, а не пользователю. `toast.success` в коде нет вовсе: успех показывается на месте — галочкой, экраном «Всё готово!», строкой «Актуальная версия».

Перенос: тосты только для сбоев, успех на месте действия; тостер выше любых гейтов и маршрутизации.

### 3.14 «Что нового» после обновления

Где: `src/components/whats-new/WhatsNewGate.tsx:8` (`WhatsNewGate`), `src/components/whats-new/releaseNotes.ts:64` (`findReleaseNoteToShow`), `src-tauri/src/settings.rs:557` (`whats_new_last_seen_version`).

Заметки о выпуске лежат в репозитории как Markdown-файлы по версиям и подключаются через `import.meta.glob`. Показывается самая новая заметка, версия которой не выше текущей и выше последней просмотренной; закрытие диалога сохраняет `whats_new_last_seen_version`. При новой установке это поле сразу получает текущую версию, поэтому новые пользователи окно не видят, а обновившиеся видят. Компонент обёрнут в `ErrorBoundary`, чтобы сбой разбора заметки не ронял приложение. В разделе «О программе» есть выключатель.

Перенос: заметки как Markdown по версиям, «последняя просмотренная» в настройках, засев при новой установке.

### 3.15 Скрытый режим отладки

Где: `src/App.tsx:118-140` (`debug_mode`), `src/components/Sidebar.tsx:65-70` (`debug_mode`).

`Cmd/Ctrl+Shift+D` переключает `debug_mode`, и это единственный способ включить его из интерфейса. Режим открывает секцию отладки (уровень логов, предпросмотр онбординга и «Что нового», выключатель обновлений, задержки вставки, просмотрщик логов в реальном времени) и мелкие детали вроде квантизации на карточках моделей. Предпросмотр онбординга рендерит настоящие компоненты с пропом `preview`, при котором все обработчики сразу возвращаются, и фиксированной кнопкой «Выйти из предпросмотра».

Перенос: настройки для опытных пользователей прятать за сочетанием клавиш; предпросмотр экранов первого запуска встраивать в сами компоненты, а не делать копии.

### 3.16 Контекстные баннеры

Где: `src/components/SecureInputWarning.tsx:50` (`impacted`), `src/components/AccessibilityPermissions.tsx:18` (`AccessibilityPermissions`).

Над содержимым любой секции могут появиться два баннера. Предупреждение о Secure Input показывается, только если какой-то шорткат действительно затронут; оно называет приложение-виновника, считает затронутые шорткаты с правильным склонением и даёт ссылку «Как исправить» и крестик. Закрытие действует до конца эпизода: когда проблема уходит и возвращается снова, баннер появляется опять.

Перенос: предупреждать только о фактическом влиянии на пользователя; закрытие привязывать к эпизоду, а не навсегда.

### 3.17 Состояния загрузки и пустоты, доступность

Загрузка — спиннер или одна приглушённая строка; пустое состояние — одно предложение, говорящее, что сделать дальше; `ErrorBoundary` при сбое рендерит `null`. Эталоном доступности служит диалог (раздел 1.9). В остальном доступность неровная: пункты сайдбара и чипы шорткатов — `div` с `onClick` без роли и `tabIndex`, `ModelCard` реагирует на Enter, но не на Space, у checkbox в `ToggleSwitch` нет доступного имени, часть `aria-label` не переведена. При переносе этого паттерна эти места надо делать кнопками.

## 4. Грабли upstream, которые не надо переносить

| Проблема                                                                                                                                                             | Где                                                                                                                                | Что делать в своём проекте                                                        |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Ошибка бэкенда `{status: "error"}` не бросается, оптимистичное значение остаётся в UI                                                                                | `src/stores/settingsStore.ts:82` (`settingUpdaters`)                                                                               | Помощник `unwrap(result)`, бросающий исключение, в каждом обновителе              |
| Откат настройки пишет снимок всего объекта и откатывает чужие изменения                                                                                              | `src/stores/settingsStore.ts:334-338` (`originalValue`)                                                                            | Функциональный `set((state) => …)` с заменой одного ключа                         |
| `useSettings()` без селектора перерисовывает все компоненты настроек на любое изменение                                                                              | `src/hooks/useSettings.ts:48` (`useSettingsStore()`)                                                                               | Селекторы на ключ, `useShallow`                                                   |
| Команды запрашивают `State<TranscriptionManager>`, а в состоянии лежит `Arc<TranscriptionManager>`; вызов упадёт с «state not managed» (сейчас их никто не вызывает) | `src-tauri/src/commands/transcription.rs:24` (`State<TranscriptionManager>`), `src-tauri/src/lib.rs:218` (`transcription_manager`) | Единое правило `State<'_, Arc<T>>` и тест, вызывающий каждую команду              |
| Непарсящийся файл настроек молча заменяется значениями по умолчанию: плагин store игнорирует ошибку загрузки                                                         | `src-tauri/src/settings.rs:1049-1052` (`default_settings`)                                                                         | Проверять загрузку файла и сохранять резервную копию перед перезаписью            |
| Методы истории `async fn` делают синхронные запросы SQLite без `spawn_blocking`                                                                                      | `src-tauri/src/managers/history.rs:195` (`get_connection`)                                                                         | `spawn_blocking` или отдельный поток БД                                           |
| `bindings.ts` обновляется только при отладочном запуске                                                                                                              | `src-tauri/src/lib.rs:774-780` (`export`)                                                                                          | Проверка в CI, что сгенерированный файл совпадает с закоммиченным                 |
| Строковые события с вручную продублированными типами                                                                                                                 | `src/lib/types/events.ts`                                                                                                          | Типизированные события specta для всего с полезной нагрузкой                      |
| Tailwind `dark:` следует ОС, а не выбранной теме                                                                                                                     | `src/App.css:6-16` (`@theme inline`)                                                                                               | `@custom-variant dark` на `data-theme`                                            |
| Тестовые скрипты лежат в дереве, но не подключены к запуску                                                                                                          | `package.json:17` (`test:keyboard`)                                                                                                | Один раннер, который находит тесты по маске                                       |
| Слушатели оверлея не отписываются, анимация исчезновения не рендерится (по чтению кода)                                                                              | `src/overlay/RecordingOverlay.tsx:134` (`setupEventListeners`), `src/overlay/RecordingOverlay.tsx:160` (`return null`)             | Возвращать очистку из эффекта; держать компонент смонтированным до конца анимации |
| Кликабельные `div` без ролей, непереведённые `aria-label`, захардкоженный английский в прогресс-баре                                                                 | `src/components/shared/ProgressBar.tsx:58` (`Downloading`)                                                                         | Кнопки вместо `div`, ключи i18n и для атрибутов доступности                       |

## 5. Что забирать в первую очередь

Для нового десктопного приложения на Tauri с настройками и фоновой работой самые окупаемые приёмы из Handy такие:

1. Трёхуровневая схема настроек «страница → группа → компонент одной настройки» с контрактом `{descriptionMode, grouped}` и строкой `SettingContainer` (разделы 1.4–1.6).
2. Типизированная карта обновителей настроек в Zustand с оптимистичным обновлением, но с исправленным откатом (раздел 1.13).
3. tauri-specta с генерацией `bindings.ts` и типизированными событиями для всего с полезной нагрузкой (разделы 1.15–1.16).
4. Токены темы парами `--light-*`/`--dark-*`, переключатель через `data-theme`, Tailwind v4 `@theme inline`, применение до рендера и событие для других окон (раздел 1.10).
5. Один источник переводов для веба и трея, lint-запрет литералов в JSX и проверка паритета ключей в CI (раздел 1.17).
6. Настройки Rust с `serde(default)`, спасением по ключам и миграциями по сырому JSON (раздел 2.10).
7. Менеджеры как `Arc<T>` в состоянии Tauri с зависимостями через конструктор и отложенная инициализация всего, что требует разрешений ОС (разделы 2.2–2.3).
8. Чистый автомат жизненного цикла на одном потоке с эффектами и `Drop`-гардом завершения (разделы 2.5–2.6).
9. Трей через снимок желаемого состояния и одного применяющего на главном потоке; правило «трей доступен» для скрытого старта и закрытия (разделы 2.14–2.15).
10. UX-мелочи: «взведённое» состояние индикатора до появления сигнала, тосты только для ошибок, подтверждение только дорогих действий, статус-точка в футере (разделы 3.5, 3.7, 3.8, 3.13).
