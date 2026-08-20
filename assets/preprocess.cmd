@echo off
setlocal
set "f="
:loop
if "%~1"=="" goto run
set "f=%~1"
shift
goto loop
:run
type "%f%"
