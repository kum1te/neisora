# Windows

Исходники Linux и Windows объединены в `main`. Версия 1.0.0 собирается в NSIS-установщик для Windows 10/11 x64; публикация зависит от успешного нативного CI. Ограничения аппаратных проверок перечислены ниже.

## Подготовлено

- JSON IPC mpv: Windows named pipes вместо Unix sockets, без TCP-порта.
- Упакованные Node, mpv и изолированный Python с yt-dlp: версии и SHA-256 закреплены в manifest, консольные окна скрыты. Системный Python не требуется.
- Защищённая DACL профиля: текущий пользователь и SYSTEM; атомарная замена существующих настроек через MoveFileExW.
- Эксклюзивная блокировка профиля LockFileEx, отказ на hardlink/reparse point; отдельный локальный activation pipe принимает только фиксированное действие показа окна, не команды или URL.
- Job Objects для дочерних процессов; внешний job приложения наследуется его новыми потомками и закрывается системой при завершении приложения. Отдельные задания ограничивают время и объём вывода.
- Нативный диалог папки и определение тома, без GTK на Windows.
- Windows SMTC: глобальные медиаклавиши, название/исполнитель, позиция и перемотка; обработчики событий снимаются при завершении. Собственные media controls mpv отключены, чтобы он не создавал вторую сессию и не обходил очередь neisora.
- Получение локальных обоев через Windows API.
- Иконка `n.` в ICO, сборка NSIS для текущего пользователя и проверка исходников Windows/MSVC в [CI](https://github.com/kum1te/neisora/actions/workflows/windows.yml).

GNU cross-check и Clippy на Linux проверяют Windows-ветви компиляции. Они не подтверждают запуск WebView2, реальное аудио, вход в Google или поведение установки. Нативные unit-тесты в Windows CI проверяют блокировку, hardlink, запись настроек, duplex pipe и ограничения процессов; Нативный production smoke запускает установленную NSIS-версию в WebView2 на одноразовой Windows VM: локальная коллекция с кириллицей/японским путём, воспроизведение mpv, seek, громкость, LRC и Next прошли проверку. В тесте используется `ao=null`, а не реальное аудиоустройство.

## Проверки сборки

```powershell
pnpm install --frozen-lockfile
python tools/prepare-runtime.py --platform windows --arch x86_64
python tools/prepare-notices.py --target x86_64-pc-windows-msvc
python tools/prepare-vulkan.py
python tools/windows-runtime-check.py
python tools/prepare-windows-installer.py
pnpm exec tauri build --config src-tauri/tauri.windows.conf.json --bundles nsis
```

GUI smoke допускается только на одноразовой Windows VM GitHub Actions и отказывается менять существующий профиль. Rust использует Windows Known Folders, а не подмену переменных APPDATA.

Загрузчику нужен 7-Zip; app-local Vulkan loader собирается CMake/MSVC из закреплённых исходников Khronos. mpv должен запускаться и без установленных GPU-драйверов. CI проверяет версии компонентов, импорт модуля cookies, реальное воспроизведение WAV с кириллицей/японским в пути, exact seek, громкость и named pipe mpv, затем сборку и тихую установку NSIS. Rust-тест отдельно запускает упакованный yt-dlp через настоящий адаптер процессов/Job Object. После полного прохождения CI установщик сохраняется в артефакте `neisora-windows-x64-test`; проверенные релизные сборки публикуются в GitHub Releases. Это online-установщик: mpv скачивается напрямую с upstream при установке с проверкой SHA-256; готовый mpv/FFmpeg не включён в распространяемый пакет.

## Проверки на реальных машинах

1. Проверить вход Google в WebView2 и импорт с отдельным тестовым аккаунтом. Современные Chromium-браузеры могут запрещать расшифровку cookies сторонним приложениям; встроенный вход остаётся основным способом.
2. Проверить реальный аудиовывод, аппаратные медиаклавиши/SMTC и ускорение GPU на Windows 10/11. Компиляция и тест через `--ao=null` не подтверждают эти сценарии.
3. Для будущего offline-установщика подготовить соответствующие исходники и лицензии готового mpv/FFmpeg. Текущий online-установщик эти бинарники не распространяет.
4. Проверить обновление/удаление NSIS и решить вопрос подписи. Без подписи возможны предупреждения Windows.

Для разработки потребуются [C++ Build Tools и WebView2](https://v2.tauri.app/start/prerequisites/). IPC реализован по [протоколу mpv](https://mpv.io/manual/stable/#json-ipc), жизненный цикл процессов — через [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).
