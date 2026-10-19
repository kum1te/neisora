# Инструменты разработки

- `install.sh` — локальная установка launcher, desktop entry и иконок.
- `regression.py` — поведенческие проверки интерфейса в WebKit через tauri-driver.
- `security.py` — отрицательные проверки IPC, XSS и CSP в изолированном профиле после `regression.py`.
- `artwork_check.py` — подбор обложки без задержки воспроизведения, один запрос на трек и сохранение оригинала при ошибке картинки.
- `visualizer_check.py` — WebGL / полноразмерный Canvas 2D, resize, потеря контекста и отказ компиляции шейдера.
- `webdriver_client.py` — вспомогательные WebDriver/IPC-команды для тестового экземпляра.

## Проверки интерфейса

Нужны `tauri-driver` и `WebKitWebDriver` в PATH. Все команды выполняются из корня репозитория. Не подключай пользовательский Google-аккаунт к тестовому профилю.

```sh
pnpm exec vite build --mode test
cargo build --locked --manifest-path src-tauri/Cargo.toml
WEBKIT_DISABLE_DMABUF_RENDERER=1 VESPER_MPV_AO=null \
  XDG_CONFIG_HOME=/tmp/neisora-test-config \
  XDG_DATA_HOME=/tmp/neisora-test-data \
  tauri-driver --port 4446 --native-port 4447
```

В другом терминале:

```sh
VESPER_WEBDRIVER_URL=http://127.0.0.1:4446 \
  VESPER_WEBDRIVER_SESSION=/tmp/neisora-test-session.json \
  python tools/regression.py --create
```

Сетевые музыкальные действия заменены фикстурами. Физика колеса проверяется с управляемыми часами кадров и настоящей геометрией WebKit. Опция `--wheel` дополнительно включает native wheel actions драйвера; их доступность зависит от WebKit и фокуса окна.

Перед обычной сборкой снова выполни `pnpm build`: тестовый IPC-перехватчик присутствует только в режиме `test` и удаляется из production.

После поведенческих тестов с теми же переменными окружения можно выполнить `python tools/security.py`. Он требует неавторизованный профиль с фикстурами, проверяет некорректные IPC-аргументы, HTML-инъекции, CSP и недоступность IPC из другого origin. Для последней проверки временный HTTP-сервер слушает только loopback; он закрывается в конце.

## Проверки аудита

- `resources.py PID --seconds 6` — CPU и память дерева процессов на Linux. Показатель CPU относится к одному логическому ядру; память PSS уменьшает повторный учёт общих страниц WebKit. Это не измерение GPU и не полный профилировщик heap.
- `lifecycle.py PID --cycles 100` — после `regression.py`: повторные открытия/закрытия большого плеера, контроль DOM, глобальных обработчиков, IPC-регистраций и кадров. PID должен принадлежать изолированному тестовому приложению.
- `queue_interaction_check.py` — после GUI-фикстур: настоящее перетаскивание мышью через xdotool, видимость иконок очереди при наведении, скрытие текста и центрирование большого плеера с сохранением выбора.
- `local_queue_check.py` — после GUI-фикстур: настоящий GTK-диалог папки (xdotool), теги и обложки WAV, mpv/LRC/история, файловый наблюдатель и смешанная очередь.
- `features_check.py` — после GUI-фикстур: лайки, разделы медиатеки, сохранение коллекции, локальная история и настоящие вызовы MPRIS через отдельную D-Bus-сессию.
- `audit_regression.py` — после `regression.py`: запоздавшие like/queue-ответы при смене трека и выходе из аккаунта, старый Next после открытия другой очереди. Скрипт восстанавливает фикстуры перед последующими тестами безопасности.

Для этих скриптов нужны те же `VESPER_WEBDRIVER_URL` и `VESPER_WEBDRIVER_SESSION`. Не запускай их против рабочего аккаунта. Подробные результаты и ограничения измерений — в `docs/AUDIT-2026-10-18.md`.

`python tools/instances.py` — Linux-проверки эксклюзивного профиля и активации окна на реальных процессах. Нужна собранная debug-версия и графическая сессия; профиль создаётся во временном каталоге, вход не импортируется. Можно передать `--binary src-tauri/target/release/neisora`. WebDriver не нужен.


## Linux release checks

`sh tools/linux-release.sh` на Ubuntu 22.04 собирает production `.deb`, `.rpm`, Arch `.pkg.tar.zst`, переносимый архив и `SHA256SUMS` в игнорируемом `release/`. Node и yt-dlp скачиваются только при сборке, проверяются по зафиксированным SHA-256; лицензии собираются в поставку. Эти файлы не коммитятся.

`dbus-run-session -- xvfb-run -a sh tools/gui-checks.sh` запускает собственный изолированный WebDriver и последовательно проверяет интерфейс, гонки, 10 000 элементов очереди, 100 открытий плеера и границы безопасности. Перед этим собери frontend в режиме `test` и Rust debug.

После установки production-пакета: `dbus-run-session -- xvfb-run -a sh tools/installed-checks.sh`. Это offline-проверка запуска, комплектности и настроек; она не подтверждает доступность YouTube. Отдельно `NEISORA_SMOKE_ONLINE=1 dbus-run-session -- xvfb-run -a sh tools/installed-checks.sh` проверяет публичный поиск, mpv, перемотку, пропуск, восстановление после SIGKILL самого mpv и завершение mpv при SIGKILL изолированного приложения. Тест использует только временный профиль и отключённый аудиовыход. В облачных CI YouTube может блокировать IP.

`node --experimental-strip-types --test tools/queue.test.ts` проверяет сохранение контекста и ограничение истории динамических очередей. `python3 tools/large_queue.py` после `audit_regression.py` проверяет DOM и клавиатуру в WebKit. Не запускай GUI-тесты одновременно в одной сессии.

Для параллельных production smoke на одной машине назначай разные пары портов: `NEISORA_SMOKE_PORT=4466` (WebDriver 4466, native driver 4467). Разные XDG-профили сами по себе не изолируют сетевые порты.
