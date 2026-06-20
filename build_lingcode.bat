@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsarm64.bat"
if errorlevel 1 exit /b 1
set "PATH=C:\Program Files\LLVM\bin;%PATH%"
cd /d "C:\Users\Xavier\lingcode_extract\lingcode_window_IDE-main"
rem --- Frugal settings for low-RAM / low-disk machine ---
set "CARGO_INCREMENTAL=0"
set "CARGO_PROFILE_DEV_DEBUG=0"
set "CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG=0"
set "CARGO_NET_RETRY=10"
rem --- Auto-update ENABLED: the built-in updater now pulls installers from GitHub Releases ---
rem --- (see crates/auto_update; release builds via bundle-windows.ps1 never set this flag). ---
rem --- To temporarily disable in a local build, uncomment the next line: ---
rem set "ZED_UPDATE_EXPLANATION=LingCode manages its own updates; built-in auto-update is disabled."
echo === clang version ===
clang --version
echo === starting cargo build (debuginfo off, -j1) ===
cargo build --bin lingcode -j 1
echo === EXIT CODE: %ERRORLEVEL% ===
exit /b %ERRORLEVEL%
