[CmdletBinding()]
param(
    [ValidateSet("x86_64", "aarch64")]
    [string] $Architecture = "x86_64",
    [switch] $Release
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$appManifest = Join-Path $root "app/Cargo.toml"
$resourceSource = Join-Path $root "app/macos/resources"
$target = "$Architecture-pc-windows-msvc"
$profile = if ($Release) { "release" } else { "debug" }

$versionLine = Select-String -Path $appManifest -Pattern '^version = "([^"]+)"$' | Select-Object -First 1
if (-not $versionLine) {
    throw "Could not read the application version from $appManifest"
}
$version = $versionLine.Matches[0].Groups[1].Value
$env:SUPERIORITY_APP_VERSION = $version
$build = ($version -split '\.')[-1]
if ($build -notmatch '^[0-9]+$') {
    throw "The patch version must be a numeric Windows build number: $version"
}

$installedTargets = rustup target list --installed
if ($installedTargets -notcontains $target) {
    throw "Missing Rust target $target. Install it with: rustup target add $target"
}

$cargoProfile = if ($Release) { @("--release") } else { @() }
& cargo build --manifest-path $appManifest --bin superiority --features rust-updater --target $target @cargoProfile
if ($LASTEXITCODE -ne 0) { throw "The Superiority Windows build failed" }
& cargo build --manifest-path (Join-Path $root "updater/Cargo.toml") --bin superiority-updater-agent --bin superiority-launcher --target $target @cargoProfile
if ($LASTEXITCODE -ne 0) { throw "The Superiority updater build failed" }

$targetDirectory = Join-Path $root "target/$target/$profile"
$application = Join-Path $targetDirectory "superiority.exe"
$agent = Join-Path $targetDirectory "superiority-updater-agent.exe"
$launcher = Join-Path $targetDirectory "superiority-launcher.exe"

function Get-SignTool {
    if ($env:SUPERIORITY_SIGNTOOL) {
        return $env:SUPERIORITY_SIGNTOOL
    }
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }
    return $null
}

function Sign-Binary([string] $Path) {
    $certificate = $env:SUPERIORITY_WINDOWS_CERTIFICATE_SHA1
    if (-not $certificate) {
        if ($Release) {
            throw "SUPERIORITY_WINDOWS_CERTIFICATE_SHA1 is required for a releasable Windows build"
        }
        Write-Warning "Leaving development binary unsigned: $Path"
        return
    }
    $signTool = Get-SignTool
    if (-not $signTool) {
        throw "signtool.exe was not found; set SUPERIORITY_SIGNTOOL to its full path"
    }
    & $signTool sign /sha1 $certificate /fd SHA256 /tr "http://timestamp.digicert.com" /td SHA256 $Path
    if ($LASTEXITCODE -ne 0) { throw "Authenticode signing failed for $Path" }
    & $signTool verify /pa $Path
    if ($LASTEXITCODE -ne 0) { throw "Authenticode verification failed for $Path" }
}

Sign-Binary $application
Sign-Binary $agent
Sign-Binary $launcher

$buildBase = Join-Path $root "build/windows/$Architecture"
$installRoot = Join-Path $buildBase "Superiority"
$versionRoot = Join-Path $installRoot "versions/$build"
$payloadRoot = Join-Path $buildBase "update-payload"
if ($installRoot -notlike "$root\build\windows\*") {
    throw "Refusing to replace an unexpected Windows build path: $installRoot"
}
Remove-Item -LiteralPath $installRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $payloadRoot -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $versionRoot, $payloadRoot -Force | Out-Null

Copy-Item -LiteralPath $launcher -Destination (Join-Path $installRoot "Superiority.exe")
Copy-Item -LiteralPath $agent -Destination (Join-Path $installRoot "superiority-updater-agent.exe")
Copy-Item -LiteralPath $application -Destination (Join-Path $versionRoot "superiority-app.exe")
Copy-Item -LiteralPath $resourceSource -Destination (Join-Path $versionRoot "resources") -Recurse

$current = [ordered]@{
    schema = 1
    version = $version
    executable = "versions/$build/superiority-app.exe"
}
$currentPath = Join-Path $installRoot "current.json"
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($currentPath, ($current | ConvertTo-Json), $utf8WithoutBom)

Copy-Item -LiteralPath $application -Destination (Join-Path $payloadRoot "superiority-app.exe")
Copy-Item -LiteralPath $launcher -Destination (Join-Path $payloadRoot "Superiority.exe")
Copy-Item -LiteralPath $agent -Destination (Join-Path $payloadRoot "superiority-updater-agent.exe")
Copy-Item -LiteralPath $resourceSource -Destination (Join-Path $payloadRoot "resources") -Recurse

$dist = Join-Path $root "dist"
New-Item -ItemType Directory -Path $dist -Force | Out-Null
$updateArchive = Join-Path $dist "Superiority-$version-$build-windows-$Architecture.zip"
$installArchive = Join-Path $dist "Superiority-$version-windows-$Architecture.zip"
Remove-Item -LiteralPath $updateArchive, $installArchive -Force -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $payloadRoot "*") -DestinationPath $updateArchive -CompressionLevel Optimal
Compress-Archive -Path $installRoot -DestinationPath $installArchive -CompressionLevel Optimal

Write-Output "Application: $installRoot"
Write-Output "Install archive: $installArchive"
Write-Output "Update archive: $updateArchive"
