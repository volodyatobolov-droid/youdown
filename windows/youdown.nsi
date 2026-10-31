; YouDown Windows installer (NSIS 3.x, per-user, no admin required).

Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!define APP_NAME "YouDown"
!define APP_EXE "YouDown.exe"
!ifndef APP_VERSION
  !define APP_VERSION "1.0.0"
!endif
!define APP_PUBLISHER "volodyatobolov-droid"
!define APP_URL "https://github.com/volodyatobolov-droid/youdown"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\YouDown"

Name "${APP_NAME}"
OutFile "../dist/YouDown-Setup-${APP_VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\YouDown"
InstallDirRegKey HKCU "Software\YouDown" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 64
BrandingText "${APP_NAME} ${APP_VERSION}"

VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "FileDescription" "YouDown installer"
VIAddVersionKey "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey "LegalCopyright" "MIT"

!define MUI_ICON "../assets/icon.ico"
!define MUI_UNICON "../assets/icon.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Запустить YouDown"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Russian"

Section "YouDown" SEC_MAIN
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "stage/YouDown.exe"
  File "stage/yt-dlp.exe"
  File "stage/ffmpeg.exe"
  File "stage/ffprobe.exe"
  File "stage/THIRD-PARTY-NOTICES.txt"
  File "stage/FFmpeg-LICENSE.txt"
  File "stage/LICENSE.txt"

  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\YouDown"
  CreateShortcut "$SMPROGRAMS\YouDown\YouDown.lnk" "$INSTDIR\${APP_EXE}"
  CreateShortcut "$SMPROGRAMS\YouDown\Удалить YouDown.lnk" "$INSTDIR\Uninstall.exe"

  WriteRegStr HKCU "Software\YouDown" "InstallDir" "$INSTDIR"

  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "YouDown"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "${APP_URL}"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" "$0"

  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

Section /o "Ярлык на рабочем столе" SEC_DESKTOP
  CreateShortcut "$DESKTOP\YouDown.lnk" "$INSTDIR\${APP_EXE}"
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_MAIN} "Приложение, yt-dlp и ffmpeg."
  !insertmacro MUI_DESCRIPTION_TEXT ${SEC_DESKTOP} "Добавить ярлык YouDown на рабочий стол."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$INSTDIR\YouDown.exe"
  Delete "$INSTDIR\yt-dlp.exe"
  Delete "$INSTDIR\ffmpeg.exe"
  Delete "$INSTDIR\ffprobe.exe"
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.txt"
  Delete "$INSTDIR\FFmpeg-LICENSE.txt"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\Uninstall.exe"

  Delete "$SMPROGRAMS\YouDown\YouDown.lnk"
  Delete "$SMPROGRAMS\YouDown\Удалить YouDown.lnk"
  RMDir "$SMPROGRAMS\YouDown"
  Delete "$DESKTOP\YouDown.lnk"

  ; Keep %APPDATA%\youdown (settings and history).
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "Software\YouDown"
  DeleteRegKey HKCU "${UNINST_KEY}"
SectionEnd
