$ErrorActionPreference = "Stop"
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host "[!] 需要管理员权限来注册输入法" -ForegroundColor Red
    Write-Host "    正在请求管理员权限..."
    Start-Process powershell -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`"" -Verb RunAs
    exit
}

$DllPath = Resolve-Path "$PSScriptRoot\..\src-tauri\target\debug\lexi_tsf.dll" -ErrorAction Stop

Write-Host ""
Write-Host "  Lexi 输入法 - TSF 注册工具" -ForegroundColor Cyan
Write-Host ""

if (-not (Test-Path $DllPath)) {
    Write-Host "[!] 找不到 DLL: $DllPath" -ForegroundColor Red
    Write-Host "    请先运行: cargo build -p lexi-tsf" -ForegroundColor Yellow
    exit 1
}

# 调用 DLL 自身的 DllRegisterServer 注册 COM + TSF 注册表项
Write-Host "[1/3] 通过 regsvr32 调用 DllRegisterServer..." -ForegroundColor Gray
$regsvr32 = "$env:SystemRoot\System32\regsvr32.exe"
$proc = Start-Process -FilePath $regsvr32 -ArgumentList @("/s", "`"$DllPath`"") -Wait -PassThru -NoNewWindow
if ($proc.ExitCode -ne 0) {
    Write-Host "[!] regsvr32 失败 (exit code: $($proc.ExitCode))" -ForegroundColor Red
    Write-Host "    尝试手动写入注册表..."
} else {
    Write-Host "    OK" -ForegroundColor Green
}

# 确保 CLSID 和 TSF 注册正确
$CLSID = "{12340001-0000-0000-C000-000000000046}"
$ProfileGuid = "{12340002-0000-0000-C000-000000000046}"

Write-Host "[2/3] 验证注册表项..." -ForegroundColor Gray

# COM 注册
$comPath = "HKLM:\SOFTWARE\Classes\CLSID\$CLSID"
$inprocPath = "$comPath\InprocServer32"
if (-not (Test-Path $comPath)) { New-Item -Path $comPath -Force | Out-Null }
Set-ItemProperty -Path $comPath -Name "(Default)" -Value "Lexi Text Service" -Type String
if (-not (Test-Path $inprocPath)) { New-Item -Path $inprocPath -Force | Out-Null }
Set-ItemProperty -Path $inprocPath -Name "(Default)" -Value $DllPath -Type String
Set-ItemProperty -Path $inprocPath -Name "ThreadingModel" -Value "Apartment" -Type String

# TSF 输入法注册
$tipPath = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$CLSID"
$profileKey = "$tipPath\LanguageProfile\0x00000804\$ProfileGuid"
if (-not (Test-Path $profileKey)) { New-Item -Path $profileKey -Force | Out-Null }
Set-ItemProperty -Path $profileKey -Name "(Default)" -Value "Lexi" -Type String
Set-ItemProperty -Path $profileKey -Name "DisplayDescription" -Value "Lexi 输入法" -Type String
Set-ItemProperty -Path $profileKey -Name "IconFile" -Value $DllPath -Type String
Set-ItemProperty -Path $profileKey -Name "IconIndex" -Value 0 -Type DWord
Set-ItemProperty -Path $profileKey -Name "Enable" -Value 1 -Type DWord

# 键盘分类
$catKey = "$tipPath\Category\{34745C63-B2F0-4784-8B67-5E12C8701A31}"
if (-not (Test-Path $catKey)) { New-Item -Path $catKey -Force | Out-Null }
Set-ItemProperty -Path $catKey -Name "(Default)" -Value $ProfileGuid -Type String

Write-Host "    OK" -ForegroundColor Green

Write-Host "[3/3] 注册完成!" -ForegroundColor Green
Write-Host ""
Write-Host "  Lexi 输入法已注册到系统。" -ForegroundColor White
Write-Host "  请在 设置 > 时间和语言 > 语言和区域 > 中文(简体) > 语言选项" -ForegroundColor White
Write-Host "  > 添加键盘 中选择 Lexi。" -ForegroundColor White
Write-Host ""
Write-Host "  注销: .\unregister-tsf.ps1" -ForegroundColor Gray
Write-Host ""
