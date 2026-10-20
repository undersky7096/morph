# Test de l'installeur sur une machine propre, dans Windows Sandbox :
#   powershell -ExecutionPolicy Bypass -File tests\sandbox\run.ps1 [-Installer chemin\Morph_x.y.z_x64-setup.exe]
# Prépare un dossier partagé (installeur, fichiers de test, inside.ps1), lance le Sandbox
# (en administrateur : Windows le demande), puis attend le rapport results\log.txt.
param(
    [string]$Installer = (Get-ChildItem "$PSScriptRoot\..\..\src-tauri\target\release\bundle\nsis\Morph_*_x64-setup.exe" |
        Sort-Object LastWriteTime | Select-Object -Last 1).FullName,
    [string]$Work = (Join-Path $env:TEMP 'morph-sandbox'),
    [int]$TimeoutMinutes = 15,
    # Installe WebView2 dans le Sandbox (absent de Windows Sandbox) pour tester l'interface.
    [switch]$InstallWebView2
)
$ErrorActionPreference = 'Stop'
if (-not $Installer -or -not (Test-Path $Installer)) { throw "Installeur introuvable : $Installer" }

Remove-Item $Work -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $Work 'results') | Out-Null
Copy-Item $Installer $Work
Copy-Item "$PSScriptRoot\inside.ps1", "$PSScriptRoot\photo.jpg", "$PSScriptRoot\son.wav" $Work
if ($InstallWebView2) { New-Item -ItemType File (Join-Path $Work 'install-webview2') | Out-Null }

$inside = 'C:\Users\WDAGUtilityAccount\Desktop\morph'
@"
<Configuration>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>$Work</HostFolder>
      <SandboxFolder>$inside</SandboxFolder>
      <ReadOnly>false</ReadOnly>
    </MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>powershell.exe -ExecutionPolicy Bypass -WindowStyle Minimized -File $inside\inside.ps1</Command>
  </LogonCommand>
</Configuration>
"@ | Out-File (Join-Path $Work 'morph.wsb') -Encoding utf8

Write-Host "Lancement de Windows Sandbox avec $(Split-Path $Installer -Leaf)…"
Start-Process "$env:windir\System32\WindowsSandbox.exe" -ArgumentList "`"$(Join-Path $Work 'morph.wsb')`"" -Verb RunAs

$log = Join-Path $Work 'results\log.txt'
$deadline = (Get-Date).AddMinutes($TimeoutMinutes)
while ((Get-Date) -lt $deadline) {
    if ((Test-Path $log) -and (Select-String -Path $log -Pattern 'FIN' -Quiet)) { break }
    Start-Sleep -Seconds 5
}
if (Test-Path $log) { Get-Content $log } else { Write-Warning 'Aucun rapport : le Sandbox ne s est pas lancé ?' }
Write-Host "Résultats : $(Join-Path $Work 'results')"
