$ErrorActionPreference = "Stop"
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Start-Process powershell -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`"" -Verb RunAs
    exit
}

$DllPath = Resolve-Path "$PSScriptRoot\..\src-tauri\target\debug\lexi_tsf.dll" -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "  Lexi 输入法 - TSF 注销工具" -ForegroundColor Cyan
Write-Host ""

# 优先调用 DllUnregisterServer
if ($DllPath -and (Test-Path $DllPath)) {
    Write-Host "[1/3] 通过 regsvr32 调用 DllUnregisterServer..." -ForegroundColor Gray
    $regsvr32 = "$env:SystemRoot\System32\regsvr32.exe"
    $proc = Start-Process -FilePath $regsvr32 -ArgumentList @("/s", "/u", "`"$DllPath`"") -Wait -PassThru -NoNewWindow
    if ($proc.ExitCode -eq 0) {
        Write-Host "    OK" -ForegroundColor Green
    } else {
        Write-Host "    regsvr32 /u 失败，回退到手动清理" -ForegroundColor Yellow
    }
} else {
    Write-Host "[1/3] DLL 不存在，直接清理注册表..." -ForegroundColor Gray
}

$CLSID = "{12340001-0000-0000-C000-000000000046}"

Write-Host "[2/3] 移除 TSF 输入处理器注册..." -ForegroundColor Gray
Remove-Item -Path "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$CLSID" -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "[3/3] 移除 COM 服务器注册..." -ForegroundColor Gray
Remove-Item -Path "HKLM:\SOFTWARE\Classes\CLSID\$CLSID" -Recurse -Force -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "  Lexi 输入法已从系统移除。" -ForegroundColor Green
Write-Host ""
