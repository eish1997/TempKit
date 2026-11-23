@echo off
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 exit /b %errorlevel%
"C:\Users\ZYF\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\rustc.exe" --test "%~dp0..\src-tauri\src\clipboard_pixels.rs" -o "%~dp0..\evidence\generation\g3\clipboard-pixels-tests.exe"
if errorlevel 1 exit /b %errorlevel%
"%~dp0..\evidence\generation\g3\clipboard-pixels-tests.exe"
