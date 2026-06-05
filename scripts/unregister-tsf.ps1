$ErrorActionPreference = "Stop"
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Start-Process powershell -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`"" -Verb RunAs
    exit
}

$CLSID = "{12340001-0000-0000-C000-000000000046}"
$ProfileGuid = "{12340001-0000-0000-C000-000000000047}"

Write-Host ""
Write-Host "  Lexi 输入法 - TSF 注销工具" -ForegroundColor Cyan
Write-Host ""

Write-Host "[1/3] 移除 TSF 输入处理器..." -ForegroundColor Gray
Remove-Item -Path "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$CLSID" -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "[2/3] 移除 COM 服务器..." -ForegroundColor Gray
Remove-Item -Path "HKLM:\SOFTWARE\Classes\CLSID\$CLSID" -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "[3/3] 注销完成!" -ForegroundColor Green
Write-Host ""
Write-Host "  Lexi 输入法已从系统移除。" -ForegroundColor White
Write-Host ""
