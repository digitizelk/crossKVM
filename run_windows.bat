@echo off
setlocal
echo ==========================================================
echo        Cross-KVM Windows Host Runner
echo ==========================================================

REM 1. Check if cargo is in PATH or in default rustup directory
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)

where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo.
    echo [NOTICE] Rust and Cargo compiler are not found on this system.
    echo Cross-KVM requires the Rust toolchain to compile native Windows binaries.
    echo.
    set /p AUTO_INSTALL="Would you like to automatically download and install Rust now? (Y/N) [default: Y]: "
    if "%AUTO_INSTALL%"=="" set AUTO_INSTALL=Y

    if /i "%AUTO_INSTALL%"=="Y" (
        echo.
        echo Downloading official rustup installer from https://win.rustup.rs...
        powershell -Command "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; (New-Object System.Net.WebClient).DownloadFile('https://win.rustup.rs/x86_64', 'rustup-init.exe')"
        if exist rustup-init.exe (
            echo Installing Rust (default profile)...
            rustup-init.exe -y
            del rustup-init.exe
            set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
        ) else (
            echo [ERROR] Failed to download rustup-init.exe. Please install manually from https://rustup.rs
            pause
            exit /b 1
        )
    ) else (
        echo Please install Rust manually from https://rustup.rs and run this script again.
        pause
        exit /b 1
    )
)

where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Cargo is still not found in PATH. Please restart your terminal or PC and run again.
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
