@echo off
setlocal
echo ==========================================================
echo        Cross-KVM Windows .EXE Binary Builder
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

echo Compiling Cross-KVM for Windows (Release mode)...
cargo build --release -p kvm-daemon
if %errorlevel% neq 0 (
    echo [ERROR] Compilation failed!
    pause
    exit /b 1
)

copy /Y target\release\kvm-daemon.exe Cross-KVM.exe >nul
echo.
echo ==========================================================
echo SUCCESS: Windows executable generated!
echo   File: %CD%\Cross-KVM.exe
echo ==========================================================
pause
