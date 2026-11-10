# Installs Marko for the current user on Windows. No administrator rights needed.
#
#   irm https://raw.githubusercontent.com/OWNER/marko/main/install.ps1 | iex
#
# With options, download first:  & .\install.ps1 -Version v0.2.0 -Associate
#   -Version v0.2.0   install that release instead of the latest
#   -Associate        make Marko an "Open with" choice for .md files
#   -Uninstall        remove what this script installed
#
# Installs to %LOCALAPPDATA%\Programs\Marko, adds it to your PATH and the Start menu.
param(
    [string]$Version = "latest",
    [switch]$Associate,
    [switch]$Uninstall
)
$ErrorActionPreference = "Stop"

$Repo = if ($env:MARKO_REPO) { $env:MARKO_REPO } else { "OWNER/marko" }
$Dir = Join-Path $env:LOCALAPPDATA "Programs\Marko"
$Shortcut = Join-Path ([Environment]::GetFolderPath("Programs")) "Marko.lnk"
$Exe = Join-Path $Dir "marko.exe"

function Set-UserPath([bool]$add) {
    $path = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @($path -split ";" | Where-Object { $_ -and ($_ -ne $Dir) })
    if ($add) { $parts += $Dir }
    [Environment]::SetEnvironmentVariable("Path", ($parts -join ";"), "User")
}

if ($Uninstall) {
    Get-Process marko -ErrorAction SilentlyContinue | Stop-Process
    Remove-Item -Recurse -Force $Dir -ErrorAction SilentlyContinue
    Remove-Item -Force $Shortcut -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force "HKCU:\Software\Classes\Applications\marko.exe" -ErrorAction SilentlyContinue
    Remove-ItemProperty "HKCU:\Software\Classes\.md\OpenWithProgids" -Name "Marko.md" -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force "HKCU:\Software\Classes\Marko.md" -ErrorAction SilentlyContinue
    Set-UserPath $false
    Write-Host "marko: removed. Your settings stay in %APPDATA%\dev.marko.app."
    return
}

$arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { throw "marko: only 64-bit Windows is supported" }
$asset = "marko-windows-$arch.zip"
$url = if ($Version -eq "latest") { "https://github.com/$Repo/releases/latest/download/$asset" }
       else { "https://github.com/$Repo/releases/download/$Version/$asset" }

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("marko-" + [guid]::NewGuid())
New-Item -ItemType Directory $tmp | Out-Null
try {
    Write-Host "marko: downloading $asset ($Version)..."
    Invoke-WebRequest -UseBasicParsing $url -OutFile (Join-Path $tmp $asset)
    Get-Process marko -ErrorAction SilentlyContinue | Stop-Process
    New-Item -ItemType Directory -Force $Dir | Out-Null
    Expand-Archive -Force (Join-Path $tmp $asset) $Dir
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($Shortcut)
$link.TargetPath = $Exe
$link.IconLocation = "$Exe,0"
$link.Description = "Marko - Markdown viewer and editor"
$link.Save()

Set-UserPath $true

if ($Associate) {
    $cmd = "`"$Exe`" `"%1`""
    New-Item -Force "HKCU:\Software\Classes\Marko.md\shell\open\command" | Out-Null
    Set-ItemProperty "HKCU:\Software\Classes\Marko.md" -Name "(default)" -Value "Markdown document"
    New-Item -Force "HKCU:\Software\Classes\Marko.md\DefaultIcon" | Out-Null
    Set-ItemProperty "HKCU:\Software\Classes\Marko.md\DefaultIcon" -Name "(default)" -Value "$Exe,0"
    Set-ItemProperty "HKCU:\Software\Classes\Marko.md\shell\open\command" -Name "(default)" -Value $cmd
    New-Item -Force "HKCU:\Software\Classes\.md\OpenWithProgids" | Out-Null
    New-ItemProperty -Force "HKCU:\Software\Classes\.md\OpenWithProgids" -Name "Marko.md" -Value "" -PropertyType String | Out-Null
    Write-Host "marko: added to 'Open with' for .md files."
}

Write-Host "marko: installed to $Dir. Open it from the Start menu, or run 'marko' in a new terminal."
