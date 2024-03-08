echo off
setlocal
cd %~dp0

if not exist obj mkdir obj
if not exist bin mkdir bin

call "%VCVARS_DIR%\vcvars32.bat"
for %%f in (src/*) do (
    cl.exe src/%%f winmm.lib /Fo"obj/" /Fe"bin/"
)
