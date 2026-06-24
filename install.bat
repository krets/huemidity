@echo off
echo ==========================================
echo Setting up HueMIDIty Environment using uv...
echo ==========================================

where uv >nul 2>nul
if %errorlevel% neq 0 (
    echo [WARNING] 'uv' was not found in your PATH.
    echo Attempting to install 'uv' via winget...
    winget install astral-sh.uv --silent --accept-source-agreements --accept-package-agreements
    
    where uv >nul 2>nul
    if %errorlevel% neq 0 (
        echo [ERROR] 'uv' could not be installed automatically.
        echo Please install it manually: winget install astral-sh.uv
        echo or visit https://github.com/astral-sh/uv
        pause
        exit /b 1
    )
)

echo Creating virtual environment...
uv venv .venv
if %errorlevel% neq 0 (
    echo [ERROR] Failed to create virtual environment.
    pause
    exit /b 1
)

echo Installing dependencies...
uv pip install -r requirements.txt
if %errorlevel% neq 0 (
    echo [ERROR] Failed to install dependencies.
    pause
    exit /b 1
)

echo.
echo ==========================================
echo Environment set up successfully!
echo To run the application:
echo .venv\Scripts\python.exe main.py
echo ==========================================
pause
