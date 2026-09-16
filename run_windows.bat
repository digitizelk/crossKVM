@echo off
setlocal
echo ==========================================================
echo        Cross-KVM Windows Host Runner
echo ==========================================================

where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Rust and Cargo are not found!
    echo Please install Rust from https://rustup.rs
    pause
    exit /b 1
)

if not exist target\release\kvm-daemon.exe (
    echo Building kvm-daemon for Windows (Release)...
    cargo build --release -p kvm-daemon
    if %errorlevel% neq 0 (
        echo [ERROR] Build failed!
        pause
        exit /b 1
    )
)

echo.
echo HOTKEYS:
echo   - Exit / Shutdown:    Ctrl + Alt + Q   or   Ctrl + Alt + C
echo   - Emergency Breakout: Triple-press Escape
echo.

set /p MAC_IP="Enter your Mac IP address [default: 192.168.1.35]: "
if "%MAC_IP%"=="" set MAC_IP=192.168.1.35

set /p POS="Is this Windows PC to the [R]ight or [L]eft of your Mac? [R/L, default: R]: "
if "%POS%"=="" set POS=R

if /i "%POS%"=="R" (
    echo Connecting to Mac at %MAC_IP%:24801 (Windows is RIGHT, Mac is LEFT)...
    target\release\kvm-daemon.exe --peer-id win-pc --left-peer mac-host --secret "my-secret-key" --connect-peer "%MAC_IP%:24801"
) else (
    echo Connecting to Mac at %MAC_IP%:24801 (Windows is LEFT, Mac is RIGHT)...
    target\release\kvm-daemon.exe --peer-id win-pc --right-peer mac-host --secret "my-secret-key" --connect-peer "%MAC_IP%:24801"
)

pause
