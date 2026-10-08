@echo off
python3 -c "import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)" >nul 2>&1
if not errorlevel 1 goto python3
python -c "import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)" >nul 2>&1
if not errorlevel 1 goto python
echo VERIFICATION_FAILED: Python 3.10+ is required (python3 or python) >&2
exit /b 127

:python3
python3 .githooks/pre_commit.py %*
exit /b %errorlevel%

:python
python .githooks/pre_commit.py %*
exit /b %errorlevel%
