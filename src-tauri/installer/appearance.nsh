; Included before MUI_PAGE_WELCOME / MUI_PAGE_FINISH by Tauri's hook.
; Only presentation: the upstream install, update and uninstall paths stay intact.
SetFont "Segoe UI" 9
!define MUI_WELCOMEPAGE_TITLE "$(NeisoraWelcomeTitle)"
!define MUI_WELCOMEPAGE_TEXT "$(NeisoraWelcomeText)"
!define MUI_FINISHPAGE_TITLE "$(NeisoraFinishTitle)"
!define MUI_FINISHPAGE_TEXT "$(NeisoraFinishText)"

LangString NeisoraWelcomeTitle 1033 "Install neisora."
LangString NeisoraWelcomeTitle 1049 "Установка neisora."
LangString NeisoraWelcomeText 1033 "Welcome to neisora.$\r$\n$\r$\nYour YouTube Music picks and local collection, together in one player.$\r$\n$\r$\nSetup installs neisora for your Windows account. An internet connection is needed to download and verify playback components.$\r$\n$\r$\nClick Next to get started."
LangString NeisoraWelcomeText 1049 "Добро пожаловать в neisora.$\r$\n$\r$\nРекомендации YouTube Music и локальная коллекция — в одном плеере.$\r$\n$\r$\nПриложение будет установлено для вашей учётной записи Windows. Для загрузки и проверки компонентов воспроизведения нужен интернет.$\r$\n$\r$\nНажмите «Далее», чтобы начать."
LangString NeisoraFinishTitle 1033 "You're all set."
LangString NeisoraFinishTitle 1049 "neisora готова."
LangString NeisoraFinishText 1033 "neisora is installed.$\r$\n$\r$\nSign in to YouTube Music for your recommendations, or open your local music collection.$\r$\n$\r$\nClick Finish to close setup."
LangString NeisoraFinishText 1049 "neisora установлена.$\r$\n$\r$\nВойдите в YouTube Music, чтобы открыть свои подборки, или добавьте папку с локальной музыкой.$\r$\n$\r$\nНажмите «Готово», чтобы закрыть установщик."
