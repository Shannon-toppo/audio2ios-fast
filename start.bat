@echo off
cd /d D:\Program\VSC\audioiosfast
if not exist target\release\audioiosfast.exe (
  echo Building release binary ^(first run only^)...
  cargo build --release
)
target\release\audioiosfast.exe
