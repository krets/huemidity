#!/bin/bash

# 1. Change the working directory to the script's location
cd "$(dirname "$0")"

echo "=========================================="
echo "Installing HueMIDI for macOS..."
echo "=========================================="

# Check if uv is installed, try to find or install it
USE_UV=true
if ! command -v uv &> /dev/null; then
    echo "'uv' not found in PATH. Checking local installation..."
    if [ -f "$HOME/.cargo/bin/uv" ]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    elif [ -f "$HOME/.local/bin/uv" ]; then
        export PATH="$HOME/.local/bin:$PATH"
    else
        echo "Attempting to install 'uv' via curl..."
        curl -LsSf https://astral.sh/uv/install.sh | sh
        export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
        
        if ! command -v uv &> /dev/null; then
            echo "[WARNING] Could not install 'uv'. Falling back to native python3 venv..."
            USE_UV=false
        fi
    fi
fi

# 2. Install local Python virtual environment
if [ "$USE_UV" = true ]; then
    echo "Creating virtual environment using uv..."
    uv venv .venv
    echo "Installing dependencies using uv..."
    uv pip install -r requirements.txt
else
    echo "Creating virtual environment using python3..."
    python3 -m venv .venv
    echo "Installing dependencies using pip..."
    ./.venv/bin/pip install --upgrade pip
    ./.venv/bin/pip install -r requirements.txt
fi

# 4. Generate a macOS .app bundle using osacompile and an AppleScript wrapper
echo "Generating macOS .app bundle..."
APP_NAME="HueMIDI"

# Create a temporary run script file
cat <<EOF > run.applescript
do shell script "cd \"$(pwd)\" && .venv/bin/python main.py > /dev/null 2>&1 &"
EOF

# Compile the AppleScript into a macOS Application Bundle
if command -v osacompile &> /dev/null; then
    rm -rf "${APP_NAME}.app"
    osacompile -o "${APP_NAME}.app" run.applescript
    rm run.applescript
    echo "Compiled ${APP_NAME}.app successfully."
else
    rm run.applescript
    echo "[WARNING] 'osacompile' is not available (this is normal if running on Windows)."
    echo "When deployed to a Mac, double-clicking this command will generate the .app."
fi

echo "=========================================="
echo "Installation complete!"
echo "You can now run the application."
echo "=========================================="
