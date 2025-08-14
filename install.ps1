# Requires: PowerShell 5+ (Windows 10/Server 2016+ recommended)
# Run in an elevated PowerShell (Run as Administrator)

[CmdletBinding()]
param(
  [string]$Repo = "hexput/main",
  [string]$AssetPattern = "x86_64-pc-windows-gnu.exe",
  [string]$InstallDir = (Join-Path $env:ProgramFiles "Hexput"),
  [string]$ExeName = "hexput-runtime.exe",
  [string]$ServiceName = "HexputRuntime",
  [string]$ServiceDisplayName = "Hexput Runtime",
  [string]$ServiceDescription = "Hexput Runtime Service",
  [switch]$PreferService
)

function Assert-Admin {
  $wi = [Security.Principal.WindowsIdentity]::GetCurrent()
  $wp = [Security.Principal.WindowsPrincipal]::new($wi)
  $isAdmin = $wp.IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
  )
  if (-not $isAdmin) {
    Write-Error "Please run this script in an elevated PowerShell (Run as " +
      "Administrator)."
    exit 1
  }
}

function Get-LatestAssetUrl {
  param(
    [string]$Repository,
    [string]$Pattern
  )
  $uri = "https://api.github.com/repos/$Repository/releases/latest"
  $headers = @{ "User-Agent" = "hexput-installer" }
  try {
    # Ensure TLS 1.2
    [Net.ServicePointManager]::SecurityProtocol =
      [Net.SecurityProtocolType]::Tls12
    $release = Invoke-RestMethod -Uri $uri -Headers $headers -ErrorAction Stop
  } catch {
    throw "Failed to fetch GitHub release info: $($_.Exception.Message)"
  }

  if (-not $release.assets) {
    throw "No assets found on the latest release for $Repository."
  }

  $asset =
    $release.assets |
    Where-Object { $_.browser_download_url -like "*$Pattern*" } |
    Select-Object -First 1

  if (-not $asset) {
    $assetNames =
      ($release.assets | ForEach-Object { $_.name }) -join ", "
    throw ("Could not find an asset matching pattern '*{0}*'. " +
      "Available assets: {1}") -f $Pattern, $assetNames
  }

  return $asset.browser_download_url
}

function Install-Binary {
  param(
    [string]$Url,
    [string]$TargetDir,
    [string]$TargetExeName
  )
  if (-not (Test-Path -LiteralPath $TargetDir)) {
    New-Item -Path $TargetDir -ItemType Directory -Force | Out-Null
  }

  $targetPath = Join-Path $TargetDir $TargetExeName
  try {
    Write-Host "Downloading: $Url"
    Invoke-WebRequest -Uri $Url -OutFile $targetPath -UseBasicParsing
    Unblock-File -LiteralPath $targetPath -ErrorAction SilentlyContinue
  } catch {
    throw "Failed to download/install binary: $($_.Exception.Message)"
  }

  return $targetPath
}

function Remove-ExistingService {
  param([string]$Name)
  $svc = Get-Service -Name $Name -ErrorAction SilentlyContinue
  if ($svc) {
    Write-Host "Removing existing service '$Name'..."
    try {
      if ($svc.Status -ne "Stopped") {
        Stop-Service -Name $Name -Force -ErrorAction SilentlyContinue
      }
    } catch { }
    Start-Sleep -Seconds 1
    sc.exe delete $Name | Out-Null
    Start-Sleep -Seconds 1
  }
}

function Try-InstallWindowsService {
  param(
    [string]$Name,
    [string]$DisplayName,
    [string]$Description,
    [string]$ExePath
  )
  try {
    New-Service `
      -Name $Name `
      -BinaryPathName "`"$ExePath`"" `
      -DisplayName $DisplayName `
      -Description $Description `
      -StartupType Automatic | Out-Null
  } catch {
    throw "New-Service failed: $($_.Exception.Message)"
  }

  $started = $false
  try {
    Start-Service -Name $Name -ErrorAction Stop
    # Wait up to ~10 seconds for Running
    for ($i = 0; $i -lt 10; $i++) {
      $svc = Get-Service -Name $Name -ErrorAction SilentlyContinue
      if ($svc -and $svc.Status -eq "Running") {
        $started = $true
        break
      }
      Start-Sleep -Seconds 1
    }
  } catch {
    # Service start failed
  }

  if (-not $started) {
    # Clean up if it didn't start
    try {
      Stop-Service -Name $Name -Force -ErrorAction SilentlyContinue
    } catch { }
    sc.exe delete $Name | Out-Null
    throw (
      "Service install attempted but the executable did not run as a " +
      "Windows service (this requires the binary to implement the " +
      "Windows service API)."
    )
  }
}

function Install-ScheduledTaskFallback {
  param(
    [string]$TaskName,
    [string]$ExePath,
    [string]$WorkingDir
  )
  # Remove any existing task
  $existing =
    schtasks.exe /Query /TN $TaskName /FO LIST /V 2>$null
  if ($LASTEXITCODE -eq 0) {
    Write-Host "Removing existing scheduled task '$TaskName'..."
    schtasks.exe /Delete /TN $TaskName /F | Out-Null
  }

  $action = New-ScheduledTaskAction `
    -Execute $ExePath `
    -WorkingDirectory $WorkingDir
  $trigger = New-ScheduledTaskTrigger -AtStartup
  $principal = New-ScheduledTaskPrincipal `
    -UserId "SYSTEM" `
    -RunLevel Highest
  $settings = New-ScheduledTaskSettingsSet `
    -RestartCount 3 `
    -RestartInterval (New-TimeSpan -Minutes 1) `
    -ExecutionTimeLimit (New-TimeSpan -Seconds 0) `
    -StartWhenAvailable `
    -AllowStartIfOnBatteries

  Register-ScheduledTask `
    -TaskName $TaskName `
    -Action $action `
    -Trigger $trigger `
    -Principal $principal `
    -Settings $settings `
    -Description "Hexput Runtime (fallback via Scheduled Task)" | Out-Null

  Start-ScheduledTask -TaskName $TaskName | Out-Null
}

# ---------------------------
# Main
# ---------------------------
Assert-Admin

Write-Host "Fetching latest release info for $Repo..."
$url = Get-LatestAssetUrl -Repository $Repo -Pattern $AssetPattern
Write-Host "Latest release asset:"
Write-Host "  $url"

$exePath = Install-Binary -Url $url -TargetDir $InstallDir `
  -TargetExeName $ExeName
Write-Host "Installed to: $exePath"

# Prefer installing as a Windows service; if it doesn't start, fall back
# to a Scheduled Task that launches at boot and restarts on failure.
$installedAs = $null
if ($PreferService.IsPresent) {
  Write-Host "PreferService flag set: attempting Windows service install..."
}

$tryService = $true
if (-not $PreferService.IsPresent) {
  # If not explicitly preferred, we still try the service route first.
  $tryService = $true
}

if ($tryService) {
  try {
    Remove-ExistingService -Name $ServiceName
    Write-Host "Creating Windows service '$ServiceName'..."
    Try-InstallWindowsService `
      -Name $ServiceName `
      -DisplayName $ServiceDisplayName `
      -Description $ServiceDescription `
      -ExePath $exePath
    $installedAs = "Service"
    Write-Host "Service installed and started."
  } catch {
    Write-Warning $_
  }
}

if (-not $installedAs) {
  $taskName = $ServiceName
  Write-Host "Falling back to Scheduled Task '$taskName'..."
  Install-ScheduledTaskFallback `
    -TaskName $taskName `
    -ExePath $exePath `
    -WorkingDir $InstallDir
  $installedAs = "ScheduledTask"
  Write-Host "Scheduled Task registered and started."
}

Write-Host "✅ Hexput installed. Mode: $installedAs"
Write-Host "Executable: $exePath"
if ($installedAs -eq "Service") {
  Write-Host "Manage with: Get-Service $ServiceName / Start-Service / " +
    "Stop-Service"
} else {
  Write-Host "Manage with: schtasks /Query /TN $ServiceName, " +
    "Start-ScheduledTask -TaskName $ServiceName, " +
    "Unregister-ScheduledTask -TaskName $ServiceName -Confirm:\$false"
}
