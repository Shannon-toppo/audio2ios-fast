@echo off
cd /d "%~dp0"
if not exist target\release\audio2ios-fast.exe (
  echo Building release binary ^(first run only^)...
  cargo build --release
)
target\release\audio2ios-fast.exe
