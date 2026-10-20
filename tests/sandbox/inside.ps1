# Exécuté dans Windows Sandbox (machine propre) à l'ouverture de session : installe Morph,
# le lance, vérifie conversion et détourage avec les fichiers installés, puis le désinstalle.
# Rapport et captures dans le dossier partagé results\ (voir run.ps1).
$root = Join-Path $env:USERPROFILE 'Desktop\morph'
$results = Join-Path $root 'results'
$log = Join-Path $results 'log.txt'
function Log($message) { "$(Get-Date -Format HH:mm:ss) $message" | Out-File -FilePath $log -Append -Encoding utf8 }
function Shot($name) {
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    [System.Drawing.Graphics]::FromImage($bitmap).CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
    $bitmap.Save((Join-Path $results "$name.png"))
}
function Run($exe, $arguments) {
    $output = & $exe @arguments 2>&1 | Out-String
    Log ("  " + ($output -replace "`r", '' -replace "`n", "`n  ").Trim())
    Log "  code de sortie : $LASTEXITCODE"
}
$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'

Log "Windows : $([Environment]::OSVersion.VersionString)"
Log "Runtime Visual C++ dans le système : msvcp140=$(Test-Path "$env:windir\System32\msvcp140.dll"), vcruntime140=$(Test-Path "$env:windir\System32\vcruntime140.dll")"

$setup = Get-ChildItem (Join-Path $root 'Morph_*_x64-setup.exe') | Select-Object -First 1
Log "Installation silencieuse : $($setup.Name)"
$process = Start-Process $setup.FullName -ArgumentList '/S' -Wait -PassThru
Log "  code de sortie : $($process.ExitCode)"
$dir = Join-Path $env:LOCALAPPDATA 'Morph'
Log "Fichiers installés : $((Get-ChildItem $dir -Name) -join ', ')"
Log "Lancement à l'ouverture de session : $((Get-ItemProperty $run -ErrorAction SilentlyContinue).Morph)"

Log 'Premier lancement (écran d accueil attendu)'
Start-Process (Join-Path $dir 'morph.exe')
Start-Sleep -Seconds 10
Log "  morph.exe tourne : $([bool](Get-Process morph -ErrorAction SilentlyContinue))"
Shot '1-premier-lancement'

$work = Join-Path $env:TEMP 'morph-test'
New-Item -ItemType Directory -Force $work | Out-Null
Copy-Item (Join-Path $root 'photo.jpg'), (Join-Path $root 'son.wav') $work
$cli = Join-Path $dir 'morph-cli.exe'
Log 'Conversion image (JPG -> WebP)'
Run $cli @('webp', (Join-Path $work 'photo.jpg'))
Log 'Conversion audio par ffmpeg (WAV -> MP3)'
Run $cli @('mp3', (Join-Path $work 'son.wav'))
Log 'Détourage (téléchargement du modèle + ONNX Runtime embarqué)'
$started = Get-Date
Run $cli @('cutout', (Join-Path $work 'photo.jpg'))
Log "  durée : $([int]((Get-Date) - $started).TotalSeconds) s"
Copy-Item (Join-Path $work '*') $results -ErrorAction SilentlyContinue
Log "Modèle présent : $(Test-Path (Join-Path $env:LOCALAPPDATA 'com.underskydev.morph\models\isnet-general-use.onnx'))"
Shot '2-apres-tests'

Log 'Désinstallation silencieuse'
Stop-Process -Name morph -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
Start-Process (Join-Path $dir 'uninstall.exe') -ArgumentList '/S' -Wait
Start-Sleep -Seconds 5
Log "  dossier d installation encore là : $(Test-Path (Join-Path $dir 'morph.exe'))"
Log "  entrée de démarrage encore là : $([bool](Get-ItemProperty $run -ErrorAction SilentlyContinue).Morph)"
Log 'FIN'
