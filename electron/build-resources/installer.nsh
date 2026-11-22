; Custom hooks injected into electron-builder's generated NSIS script
; via `nsis.include` in package.json. electron-builder includes this
; file in the header it shares between the installer and the
; uninstaller, ahead of its own installer.nsi.
;
; Two jobs: give the install an ffmpeg.exe without the installer
; carrying one, and the optional purge of per-user data on uninstall.
;
; ffmpeg
; ======
; Downloads need ffmpeg. The installer does not carry it: gyan.dev's
; build is ~90 MB zipped, and it is GPL, so redistributing it would
; oblige an offer of corresponding source that cannot be made (the
; build does not publish the versions of the libraries it links). The
; installer downloads it from gyan.dev while it installs, into
; $INSTDIR\resources\bin\ beside the binaries the package does carry,
; which is where the backend looks first (AppState::bundled_bin).
;
; The pin below is declared once more in scripts/fetch-windows-deps.mjs
; (`INSTALL_TIME_DEPS`), which the credits and the README read;
; tests/arch/windows_deps.sh holds these lines to that entry verbatim.
;
; Keeping the copy across updates
; -------------------------------
; Through 0.18.0 every update deleted the downloaded copy and fetched
; it again, and an update run offline ended with none. The cause is
; electron-builder's upgrade order (app-builder-lib 26.18.0,
; templates/nsis/installSection.nsh, the same line for line in 25.1.8,
; as is include/installUtil.nsh): it runs the previous version's
; uninstaller (`uninstallOldVersion`, line 52, which execs it with
; `/S /KEEP_APP_DATA --updated` — include/installUtil.nsh lines 206 and
; 224; an installer launched with --delete-app-data passes that flag
; instead of --updated, lines 202-203, and the update then behaves as
; an uninstall followed by a fresh install, ffmpeg download included:
; step 1 below sets nothing aside then),
; and that uninstaller empties $INSTDIR. It is the uninstaller the
; previous version shipped, built with 25.1.8 through 0.18.0 and with
; 26.18.0 from this version on, and both remove the tree the same way:
; under --updated they move it aside and then remove it, or, when a
; file is busy, move it back and fail (26.18.0 uninstaller.nsh lines
; 164-187; 25.1.8 lines 148-169). The installer retries a failed
; uninstaller and, once the user cancels, stops the update
; (installUtil.nsh lines 215-239 and 128-132). The two differ only in
; where `customUnInstall` runs: 25.1.8 runs it after that removal
; (line 239), 26.18.0 before it (lines 156-157). `customInstall` only
; runs after the new files land (installSection.nsh line 82), by which
; time the copy is gone. Releases through 0.18.0 also named it in a
; `Delete` in their customUnInstall.
;
; So the copy is set aside before that uninstaller runs, and put back
; after the new files land:
;
;   1. `customHeader` (inserted at installer.nsi line 46, before the
;      "install" section is defined at line 94) defines a hidden section
;      of its own. NSIS runs sections in the order they are defined, so
;      it runs immediately before "install", in the same process and
;      after every page: $INSTDIR is final, and a user who cancels on a
;      page never reaches it. It copies ffmpeg.exe and its stamp into
;      FFMPEG_STAGING, beside $INSTDIR, which an uninstaller run by an
;      update (--updated) leaves alone. Under --delete-app-data it sets
;      nothing aside and removes any FFMPEG_STAGING it finds: an
;      uninstaller from 0.18.0 or earlier would leave that folder, and
;      step 2 would restore it into what should be a fresh install.
;
;      Why not `customInit` (.onInit, installer.nsi line 80): it runs
;      before the install-mode and directory pages, so $INSTDIR is not
;      final yet and the user can still cancel; and in a per-machine
;      install it also runs in the outer, unelevated instance. Why not
;      `customCheckAppRunning`: installSection.nsh lines 35-37 skip it
;      in the elevated inner instance of an assisted install, which is
;      the instance that does a per-machine install.
;
;      Per-machine (UAC): the install-mode page elevates by relaunching
;      the installer (multiUserUi.nsh lines 152-159) and the outer
;      instance quits once the inner one is done, so sections, this one
;      included, run only in the elevated instance. The one exception
;      is a silent update of a per-machine install started unelevated:
;      the "install" section relaunches elevated and quits (installer.nsi
;      lines 101-115), after this section has already run unelevated.
;      This section skips that case — it could not write $INSTDIR
;      anyway — and the elevated instance runs it. That elevated
;      instance switches to the per-machine install only inside the
;      "install" section (installer.nsi line 117), after this section,
;      so this section makes the same switch first, under the same
;      condition (installer.nsi lines 101-102: a per-machine install
;      exists and the run is silent); otherwise a machine with both a
;      per-user and a per-machine install would set aside the wrong one.
;
;   2. `customInstall` puts the copy back once the new files are in
;      place, then decides whether to download (below).
;
; Setting aside copies rather than moves, so an install that stops
; between the two steps (the app-running prompt cancelled, the old
; uninstaller failing on a busy file, which moves its tree back first —
; both end in `Quit`) leaves the existing install whole. One exception:
; a per-machine update also runs the per-user install's uninstaller,
; after the per-machine one has succeeded (installSection.nsh lines
; 55-57), and if that second one fails the update stops with the
; per-machine folder already removed; installing to that folder again
; restores the copy. The stale staging copy is replaced or restored by
; the next install, and a real uninstall by this version's uninstaller
; removes it. One left by an aborted upgrade from 0.18.0 or earlier, and then
; uninstalled with that version's uninstaller, stays behind: those
; uninstallers do not know about it.
;
; Only the same install is kept. Switching between per-user and
; per-machine, or picking another folder, is a new $INSTDIR, and that
; install downloads ffmpeg like a fresh one.
;
; When to download
; ----------------
; A stamp file beside ffmpeg.exe records the pinned version it was
; fetched at. After the restore:
;
;   - ffmpeg.exe present, stamp equals FFMPEG_VERSION   → keep it.
;   - ffmpeg.exe present, stamp names another version   → download; if
;     that fails, the old copy and its stamp stay, so the next install
;     tries again.
;   - ffmpeg.exe restored, but its stamp not            → record it as
;     "unknown" and download; if that fails, the "unknown" stamp stays,
;     so the next install tries again. Taken for unstamped, the copy
;     would read as 7.1.1 whatever it is. (Should writing that stamp
;     fail too, and then the download, the next install does take it
;     for unstamped.)
;   - ffmpeg.exe present, no stamp                      → treat it as
;     FFMPEG_UNSTAMPED_VERSION, write that stamp, and compare as above.
;     Every installer released, 0.1.1 through 0.18.0, pinned 7.1.1 and wrote
;     no stamp, so an unstamped copy they fetched is 7.1.1. The other
;     way to have one is a copy the user put there by hand, as the old
;     failure message suggested; that is kept too, until a release pins
;     a different version.
;   - no ffmpeg.exe, but one on PATH                    → nothing to do;
;     the app finds that one.
;   - no ffmpeg.exe at all                              → download.
;
; Download failures are not fatal: playback works without ffmpeg, only
; downloads need it. Running the installer again online retries.
;
; Integrity: HTTPS from GitHub protects the download end to end; the
; installer does not check a hash, because PowerShell-quoted
; Get-FileHash inside nsExec is fragile to escape.

!define FFMPEG_VERSION "7.1.1"
!define FFMPEG_URL "https://github.com/GyanD/codexffmpeg/releases/download/7.1.1/ffmpeg-7.1.1-essentials_build.zip"
!define FFMPEG_ARCHIVE_PATH "ffmpeg-7.1.1-essentials_build\bin\ffmpeg.exe"
; What an unstamped copy is taken to be: the build every installer
; released, 0.1.1 through 0.18.0, fetched. Fixed history, not the pin.
!define FFMPEG_UNSTAMPED_VERSION "7.1.1"

!define FFMPEG_DIR "$INSTDIR\resources\bin"
!define FFMPEG_EXE "$INSTDIR\resources\bin\ffmpeg.exe"
!define FFMPEG_STAMP "$INSTDIR\resources\bin\ffmpeg.version"
; Beside $INSTDIR, not in it: on the same volume, so the restore is a
; rename, and outside the directory the old uninstaller empties.
!define FFMPEG_STAGING "$INSTDIR-ffmpeg-staging"

!macro customHeader
    !ifndef BUILD_UNINSTALLER
        ; Step 1 above: set ffmpeg aside before the "install" section
        ; runs the old uninstaller.
        Section "-ffmpegSetAside"
            Var /GLOBAL ffmpegSetAsideHere
            StrCpy $ffmpegSetAsideHere "1"
            ${If} $installMode == "all"
            ${AndIfNot} ${UAC_IsAdmin}
                StrCpy $ffmpegSetAsideHere "0"
            ${EndIf}
            ; The silent per-machine update, as installer.nsi lines
            ; 101-117 will handle it: elevated, switch to the per-machine
            ; install now; unelevated, leave it to the elevated instance.
            ; Under the template's own guards (installer.nsi lines 99-100),
            ; outside which these variables and macros do not exist.
            !ifndef INSTALL_MODE_PER_ALL_USERS
                !ifndef ONE_CLICK
                    ${If} $hasPerMachineInstallation == "1"
                    ${AndIf} ${Silent}
                        ${If} ${UAC_IsAdmin}
                            !insertmacro setInstallModePerAllUsers
                        ${Else}
                            StrCpy $ffmpegSetAsideHere "0"
                        ${EndIf}
                    ${EndIf}
                !endif
            !endif
            ; An installer run with --delete-app-data makes the update an
            ; uninstall followed by a fresh install, ffmpeg download
            ; included: nothing is set aside, and a staging folder an
            ; interrupted install left is dropped rather than restored.
            ; An uninstaller from 0.18.0 or earlier would not remove
            ; that folder, so this cannot be left to the uninstaller.
            ;
            ; `isDeleteAppData` is electron-builder's own test of the flag
            ; (its script generator defines it ahead of this file, from
            ; StdUtils.TestParameter), the same one uninstallOldVersion
            ; asks before passing --delete-app-data on, so the two cannot
            ; disagree about how the flag was spelled.
            ${If} $ffmpegSetAsideHere == "0"
                ; About to hand the install to an elevated instance,
                ; which runs this section again.
                DetailPrint "ffmpeg: the elevated installer sets it aside."
            ${ElseIf} ${isDeleteAppData}
                RMDir /r "${FFMPEG_STAGING}"
                DetailPrint "ffmpeg: --delete-app-data; the installed copy is not kept, and ffmpeg is fetched as for a fresh install."
            ${ElseIf} ${FileExists} "${FFMPEG_EXE}"
                DetailPrint "ffmpeg: setting the installed copy aside for the update..."
                RMDir /r "${FFMPEG_STAGING}"
                CreateDirectory "${FFMPEG_STAGING}"
                ClearErrors
                CopyFiles /SILENT "${FFMPEG_EXE}" "${FFMPEG_STAGING}\ffmpeg.exe"
                ${If} ${FileExists} "${FFMPEG_STAMP}"
                    CopyFiles /SILENT "${FFMPEG_STAMP}" "${FFMPEG_STAGING}\ffmpeg.version"
                ${EndIf}
                ${If} ${Errors}
                    ; A partial copy is worse than none: drop it, and
                    ; let customInstall download as for a fresh install.
                    DetailPrint "ffmpeg: could not set it aside; it will be downloaded again."
                    RMDir /r "${FFMPEG_STAGING}"
                ${EndIf}
            ${EndIf}
        SectionEnd
    !endif
!macroend

!macro customInstall
    Var /GLOBAL ffmpegRecorded
    ; Set when the restored copy's stamp could not be put back: its
    ; version is then unknown, and the copy must not pass for an
    ; unstamped one.
    Var /GLOBAL ffmpegStampLost
    StrCpy $ffmpegStampLost 0

    ; --- Step 2: put back what step 1 set aside -------------------
    ; Only when the new files did not bring one: nothing in the package
    ; does, and a copy already in place is not ours to replace here.
    ${If} ${FileExists} "${FFMPEG_STAGING}\ffmpeg.exe"
    ${AndIfNot} ${FileExists} "${FFMPEG_EXE}"
        CreateDirectory "${FFMPEG_DIR}"
        Delete "${FFMPEG_STAMP}"
        ClearErrors
        Rename "${FFMPEG_STAGING}\ffmpeg.exe" "${FFMPEG_EXE}"
        ${If} ${Errors}
            ClearErrors
            CopyFiles /SILENT "${FFMPEG_STAGING}\ffmpeg.exe" "${FFMPEG_EXE}"
        ${EndIf}
        ${If} ${Errors}
            ; A partial copy beside a matching stamp would never be
            ; replaced: drop it, and download as for a fresh install.
            Delete "${FFMPEG_EXE}"
            DetailPrint "ffmpeg: could not restore the previous copy; it will be downloaded again."
        ${Else}
            ${If} ${FileExists} "${FFMPEG_STAGING}\ffmpeg.version"
                ClearErrors
                Rename "${FFMPEG_STAGING}\ffmpeg.version" "${FFMPEG_STAMP}"
                ${If} ${Errors}
                    ClearErrors
                    CopyFiles /SILENT "${FFMPEG_STAGING}\ffmpeg.version" "${FFMPEG_STAMP}"
                ${EndIf}
                ${If} ${Errors}
                    ; Without its stamp the copy would read as an
                    ; unstamped 7.1.1, whatever it is.
                    Delete "${FFMPEG_STAMP}"
                    StrCpy $ffmpegStampLost 1
                    DetailPrint "ffmpeg: restored the previous copy but not its version stamp; it will be downloaded again."
                ${EndIf}
            ${EndIf}
            ${If} $ffmpegStampLost == 0
                DetailPrint "ffmpeg: kept the copy from the previous install."
            ${EndIf}
        ${EndIf}
    ${EndIf}
    RMDir /r "${FFMPEG_STAGING}"

    ; --- Decide whether to download -------------------------------
    ${If} ${FileExists} "${FFMPEG_EXE}"
        StrCpy $ffmpegRecorded ""
        ${If} ${FileExists} "${FFMPEG_STAMP}"
            ClearErrors
            FileOpen $0 "${FFMPEG_STAMP}" r
            ${IfNot} ${Errors}
                FileRead $0 $ffmpegRecorded
                FileClose $0
            ${EndIf}
        ${ElseIf} $ffmpegStampLost == 1
            ; A version that matches no pin: the copy is replaced, and
            ; should the download fail, the next install tries again
            ; rather than taking the copy for an unstamped one.
            StrCpy $ffmpegRecorded "unknown"
            FileOpen $0 "${FFMPEG_STAMP}" w
            FileWrite $0 "unknown"
            FileClose $0
        ${Else}
            StrCpy $ffmpegRecorded "${FFMPEG_UNSTAMPED_VERSION}"
            FileOpen $0 "${FFMPEG_STAMP}" w
            FileWrite $0 "${FFMPEG_UNSTAMPED_VERSION}"
            FileClose $0
        ${EndIf}
        ${If} $ffmpegRecorded == "${FFMPEG_VERSION}"
            DetailPrint "ffmpeg ${FFMPEG_VERSION} is already installed; not downloading it."
            Goto ffmpeg_done
        ${EndIf}
        DetailPrint "ffmpeg: the installed copy is '$ffmpegRecorded'; this release pins ${FFMPEG_VERSION}."
        Goto ffmpeg_fetch
    ${EndIf}

    ; No copy of our own. Users who installed ffmpeg via winget, scoop
    ; or a standalone build may have it on PATH, and the app finds it
    ; there; `where` exits 0 when it finds at least one match.
    DetailPrint "Checking for an existing ffmpeg on PATH..."
    nsExec::ExecToStack 'cmd /c where ffmpeg.exe'
    Pop $0
    Pop $1
    StrCmp $0 "0" 0 ffmpeg_fetch
    DetailPrint "ffmpeg is already on PATH; not downloading it."
    Goto ffmpeg_done

    ffmpeg_fetch:
        DetailPrint "Fetching ffmpeg ${FFMPEG_VERSION} (~90 MB; required for downloads)..."

    ; $PLUGINSDIR is a temp dir NSIS creates and cleans on install
    ; finish, so the zip and the extracted tree don't linger after the
    ; one binary is copied out.
    InitPluginsDir

    ; inetc's /POPUP opens a per-download progress dialog, so the user
    ; can see the install is still alive during the download. A failure
    ; does not abort the install.
    inetc::get /CAPTION "Downloading ffmpeg ${FFMPEG_VERSION}" /POPUP "" "${FFMPEG_URL}" "$PLUGINSDIR\ffmpeg.zip" /END
    Pop $0
    StrCmp $0 "OK" ffmpeg_extract ffmpeg_failed

    ; The zip nests the binary under FFMPEG_ARCHIVE_PATH. Expand-Archive
    ; lays the tree out in the plug-in temp dir, and only ffmpeg.exe is
    ; copied out of it.
    ffmpeg_extract:
        DetailPrint "Extracting ffmpeg.exe..."
        nsExec::ExecToStack 'powershell -NoProfile -ExecutionPolicy Bypass -Command "Expand-Archive -Path $\"$PLUGINSDIR\ffmpeg.zip$\" -DestinationPath $\"$PLUGINSDIR\ffmpeg-extract$\" -Force"'
        Pop $0
        Pop $1
        StrCmp $0 "0" ffmpeg_install 0
        DetailPrint "ffmpeg extraction failed (exit=$0). Continuing without a new copy."
        Goto ffmpeg_done

    ffmpeg_install:
        CreateDirectory "${FFMPEG_DIR}"
        ClearErrors
        CopyFiles /SILENT "$PLUGINSDIR\ffmpeg-extract\${FFMPEG_ARCHIVE_PATH}" "${FFMPEG_EXE}"
        ${If} ${Errors}
            DetailPrint "ffmpeg: could not copy ffmpeg.exe into ${FFMPEG_DIR}."
            Goto ffmpeg_done
        ${EndIf}
        ; The stamp is written only once the binary is in place, so a
        ; failed update leaves the old stamp and the next install retries.
        FileOpen $0 "${FFMPEG_STAMP}" w
        FileWrite $0 "${FFMPEG_VERSION}"
        FileClose $0
        DetailPrint "ffmpeg ${FFMPEG_VERSION} installed at ${FFMPEG_EXE}"
        Goto ffmpeg_done

    ffmpeg_failed:
        ${If} ${FileExists} "${FFMPEG_EXE}"
            DetailPrint "ffmpeg download failed: $0. Keeping the installed copy; run the installer again with an internet connection to update it."
        ${Else}
            DetailPrint "ffmpeg download failed: $0. ani-gui installs anyway; playback works without ffmpeg, only downloads need it. To retry, run the installer again with an internet connection."
        ${EndIf}

    ffmpeg_done:
!macroend

!macro customUnInstall
    ; A real uninstall removes any staging copy an interrupted install
    ; left behind. It sits beside $INSTDIR, not in it, so only this
    ; removes it. ffmpeg and its stamp are left to the standard sequence,
    ; which removes the whole of $INSTDIR right after this macro
    ; (26.18.0 uninstaller.nsh lines 156-157, then `RMDir /r $INSTDIR` at
    ; line 187). Nothing here removes under $INSTDIR: on an update that
    ; sequence moves the tree back and fails when a file is busy, and a
    ; deletion made here would stay made. tests/arch/windows_deps.sh
    ; check 9 holds this macro to that.
    ;
    ; Not during an update. A newer installer runs this uninstaller with
    ; --updated after it has set the copy aside in that same staging
    ; directory, and removing it here would delete the very copy the
    ; update is about to restore.
    ${IfNot} ${isUpdated}
        RMDir /r "${FFMPEG_STAGING}"
    ${EndIf}

    ; Everything below lives in the user's own profile, whatever the
    ; install mode. A per-machine install runs this in the all-users
    ; shell context, where $APPDATA and $LOCALAPPDATA name
    ; C:\ProgramData, so switch to the current user's context for the
    ; removals and back once they are done, as electron-builder's own
    ; app-data removal does (uninstaller.nsh: "electron always uses per
    ; user app data"). The installer writes its copy below in the
    ; current user's context too.
    ${If} $installMode == "all"
        SetShellVarContext current
    ${EndIf}

    ; Every install leaves a copy of its installer here
    ; (electron-builder's APP_INSTALLER_STORE_FILE, kept for
    ; electron-updater, which this app does not use), and nothing in
    ; electron-builder's sequence removes it. An update runs the
    ; previous version's uninstaller with --updated before the new
    ; installer writes its own copy, so removing it then would be
    ; harmless; it is kept anyway, so an update never deletes a folder
    ; an updater would download into.
    ${IfNot} ${isUpdated}
        RMDir /r "$LOCALAPPDATA\ani-gui-updater"
    ${EndIf}

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
        ; %APPDATA%\thirdmovement\ani-gui\ for config. Electron keeps its
        ; own profile in %APPDATA%\ani-gui\ (the Chromium profile, window
        ; state, and the tracker sign-ins under tokens\), named for the
        ; app's production name in electron/main.js; a Yes removes that
        ; too. The dev profile, ani-gui-dev, is not the installed app's
        ; and is left alone.
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
        ; The publisher folder holding those two goes as well, but only
        ; once empty: other apps by the same publisher keep their data
        ; there, and so does the dev profile, ani-gui-dev. RMDir without
        ; /r removes nothing from a folder that still holds anything.
        ; tests/arch/windows_uninstall.sh refuses a recursive removal of
        ; any thirdmovement path but the app's own.
        RMDir "$LOCALAPPDATA\thirdmovement"
        RMDir "$APPDATA\thirdmovement"
        DetailPrint "Removing the app profile under $APPDATA\ani-gui..."
        RMDir /r "$APPDATA\ani-gui"
        Goto purge_done

    purge_done:
    ${If} $installMode == "all"
        SetShellVarContext all
    ${EndIf}
!macroend
