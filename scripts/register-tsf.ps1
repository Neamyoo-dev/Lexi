$ErrorActionPreference = "Stop"
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host "[!] 需要管理员权限来注册输入法" -ForegroundColor Red
    Write-Host "    正在请求管理员权限..."
    Start-Process powershell -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`"" -Verb RunAs
    exit
}

$CLSID = "{12340001-0000-0000-C000-000000000046}"
$ProfileGuid = "{12340001-0000-0000-C000-000000000047}"
$DllPath = "$PSScriptRoot\..\src-tauri\target\debug\lexi_tsf.dll"

Write-Host ""
Write-Host "  Lexi 输入法 - TSF 注册工具" -ForegroundColor Cyan
Write-Host ""

if (-not (Test-Path $DllPath)) {
    Write-Host "[!] 找不到 DLL: $DllPath" -ForegroundColor Red
    Write-Host "    请先运行: cargo build -p lexi-tsf" -ForegroundColor Yellow
    exit 1
}

Write-Host "[1/3] 注册 COM 服务器..." -ForegroundColor Gray
$regPath = "HKLM:\SOFTWARE\Classes\CLSID\$CLSID"
$inprocPath = "$regPath\InProcServer32"

if (-not (Test-Path $regPath)) { New-Item -Path $regPath -Force | Out-Null }
Set-ItemProperty -Path $regPath -Name "(Default)" -Value "Lexi Text Service" -Type String

if (-not (Test-Path $inprocPath)) { New-Item -Path $inprocPath -Force | Out-Null }
Set-ItemProperty -Path $inprocPath -Name "(Default)" -Value $DllPath -Type String
Set-ItemProperty -Path $inprocPath -Name "ThreadingModel" -Value "Apartment" -Type String

Write-Host "[2/3] 注册 TSF 输入处理器..." -ForegroundColor Gray
$profilePath = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$CLSID"
if (-not (Test-Path $profilePath)) { New-Item -Path $profilePath -Force | Out-Null }

$profileSubKey = "$profilePath\LanguageProfile\0x00000804"
if (-not (Test-Path $profileSubKey)) { New-Item -Path $profileSubKey -Force | Out-Null }

Set-ItemProperty -Path $profileSubKey -Name "Profile" -Value $ProfileGuid -Type String
Set-ItemProperty -Path $profileSubKey -Name "Description" -Value "Lexi" -Type String
Set-ItemProperty -Path $profileSubKey -Name "DisplayDescription" -Value "Lexi 输入法" -Type String
Set-ItemProperty -Path $profileSubKey -Name "IconFile" -Value $DllPath -Type String
Set-ItemProperty -Path $profileSubKey -Name "IconIndex" -Value 0 -Type DWord
Set-ItemProperty -Path $profileSubKey -Name "Enable" -Value 1 -Type DWord

$categoryItem = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$CLSID\Category"
$catKeyboard = "$categoryItem\Category\$ProfileGuid"
if (-not (Test-Path $catKeyboard)) { New-Item -Path $catKeyboard -Force | Out-Null }
Set-ItemProperty -Path $catKeyboard -Name "CategoryGuid" -Value "{34745C63-B2F0-4784-8B67-5E12C8701A31}" -Type String

$catDisplay = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$CLSID\Category\Attribute\{34745C63-B2F0-4784-8B67-5E12C8701A31}\{12340001-0000-0000-C000-000000000048}"
if (-not (Test-Path $catDisplay)) { New-Item -Path $catDisplay -Force | Out-Null }

Write-Host "[3/3] 注册完成!" -ForegroundColor Green
Write-Host ""
Write-Host "  Lexi 输入法已注册到系统。" -ForegroundColor White
Write-Host "  请在 设置 > 时间和语言 > 语言和区域 > 中文(简体) > 语言选项" -ForegroundColor White
Write-Host "  > 添加键盘 中选择 Lexi。" -ForegroundColor White
Write-Host ""
Write-Host "  使用本脚本的另一半来注销:" -ForegroundColor Gray
Write-Host "  .\unregister-tsf.ps1" -ForegroundColor Gray
Write-Host ""
