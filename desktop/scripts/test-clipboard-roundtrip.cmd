@echo off
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 exit /b %errorlevel%
set "PATH=C:\Users\ZYF\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin;%PATH%"
set "CARGO_HOME=%~dp0..\.cargo-home"
set "CARGO_TARGET_DIR=%~dp0..\src-tauri\target\generation"
cargo build --manifest-path "%~dp0..\src-tauri\Cargo.toml" --release --offline --bin quickgen-clipboard-probe
if errorlevel 1 exit /b %errorlevel%
"%~dp0..\src-tauri\target\generation\release\quickgen-clipboard-probe.exe"
