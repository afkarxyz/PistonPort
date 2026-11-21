@echo off
setlocal
python -B "%~dp0run_native.py"
exit /b %errorlevel%
