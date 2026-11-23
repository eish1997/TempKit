@echo off
call "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64
if errorlevel 1 exit /b %errorlevel%
set "PATH=C:\Users\ZYF\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin;%PATH%"
set "CARGO_HOME=%~dp0..\.cargo-home"
cd /d "%~dp0.."
call npm.cmd run %*
