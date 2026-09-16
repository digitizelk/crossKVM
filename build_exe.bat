@echo off
setlocal
echo ==========================================================
echo        Cross-KVM Windows .EXE Binary Builder
echo ==========================================================

where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Rust and Cargo are not found!
    echo Please install Rust from https://rustup.rs
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
