; Custom uninstall hook injected into electron-builder's generated
; NSIS script via `nsis.include` in package.json.
;
; Its one job is the optional purge of per-user data below.
;
; This file fetches nothing at install time, and must not start to
; (tests/arch/windows_deps.sh). Through 0.17.0 a customInstall macro
; here downloaded ffmpeg into $INSTDIR\resources\bin. electron-builder
; runs the previous version's uninstaller before an upgrade lands, and
; that empties $INSTDIR, so every update deleted the fetched copy and
; downloaded it again, and an update run offline left none. The
; installer carries ffmpeg in its payload now, staged by
; scripts/fetch-windows-deps.mjs like the other bundled binaries, and
; the standard uninstall sequence removes it with the rest.

!macro customUnInstall
    ; Skip the purge prompt during the silent pre-upgrade uninstall.
    ; electron-builder runs the previous version's uninstaller with
    ; /S before installing the new one; surfacing a "delete settings?"
    ; dialog there is misleading because the user picked Update, not
    ; Uninstall. Interactive uninstalls (Add/Remove Programs, manual
    ; Uninst.exe run) are not silent and still hit the prompt.
    ${IfNot} ${Silent}
        ; Optional: purge the per-user data dirs the running app writes to
        ; (cache.sqlite with play resolutions + image bytes, config.toml,
        ; the watch history, log dir). NSIS would never touch these on its
        ; own because they live outside $INSTDIR — they're created at
        ; runtime via the `directories` Rust crate (ProjectDirs::from(
        ; "net", "thirdmovement", "ani-gui")), which on Windows resolves
        ; to %LOCALAPPDATA%\thirdmovement\ani-gui\ for cache/data and
        ; %APPDATA%\thirdmovement\ani-gui\ for config.
        ;
        ; Default is No: a "reinstall to fix something" cycle should
        ; preserve the user's settings + cached resolutions (instant
        ; playback for previously-watched episodes is the main UX win).
        ; Explicit Yes wipes both trees so a deliberate "I'm done with
        ; this app" uninstall leaves nothing behind. MB_ICONQUESTION +
        ; MB_DEFBUTTON2 makes No the highlighted choice if the user just
        ; mashes Enter.
        MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 \
            "Also delete your ani-gui settings, cache, and watch history?$\r$\n$\r$\nChoose No to keep them — handy if you plan to reinstall." \
            IDYES purge_user_data IDNO purge_done
    ${EndIf}
    Goto purge_done

    purge_user_data:
        DetailPrint "Removing user data under $LOCALAPPDATA\thirdmovement\ani-gui..."
        RMDir /r "$LOCALAPPDATA\thirdmovement\ani-gui"
        DetailPrint "Removing user data under $APPDATA\thirdmovement\ani-gui..."
        RMDir /r "$APPDATA\thirdmovement\ani-gui"
        Goto purge_done

    purge_done:
!macroend
