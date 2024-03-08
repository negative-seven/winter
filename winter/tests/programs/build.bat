echo off
setlocal
cd %~dp0

if not exist obj mkdir obj
if not exist bin mkdir bin

call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars32.bat"
for %%f in (src/*) do (
    cl.exe src/%%f winmm.lib /Fo"obj/" /Fe"bin/"
)
