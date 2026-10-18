# hvolt installer for Windows: downloads the prebuilt zip from the GitHub
# release, verifies it against SHA256SUMS.txt, and installs hvolt.exe.
#
#   irm https://raw.githubusercontent.com/hvolt-project/hvolt/main/install.ps1 | iex
#
# Parameters: -Version (without the leading "v", default: latest)
#             -Prefix  (install directory, default: $env:LOCALAPPDATA\hvolt\bin)
param(
    [string]$Version = "latest",
    [string]$Prefix = "$env:LOCALAPPDATA\hvolt\bin"
)

$ErrorActionPreference = "Stop"
$repo = "hvolt-project/hvolt"
$key = "win32-x64"

# Release assets are versioned, so "latest" has to be resolved to its tag first.
if ($Version -eq "latest") {
    # Resolve through the API: the HTML redirect from /releases/latest does not
    # carry the tag for anonymous requests.
    $release = Invoke-RestMethod -UseBasicParsing `
        -Uri "https://api.github.com/repos/$repo/releases/latest" `
        -Headers @{ "User-Agent" = "hvolt-installer" }
    $tag = $release.tag_name
    if (-not $tag) { throw "could not resolve the latest release" }
} else {
    $tag = "v$Version"
}
$bare = $tag.TrimStart("v")
$base = "https://github.com/$repo/releases/download/$tag"
$asset = "hvolt-$bare-$key.zip"

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("hvolt-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
    # Asset downloads: release URL first, then the API asset endpoint, which works
    # where github.com is blocked but api.github.com is reachable.
    function Fetch-Asset([string]$Name, [string]$Out) {
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$Name" -OutFile $Out
            return
        } catch {
            Write-Verbose "release URL failed for $Name, trying the API"
        }
        $rel = Invoke-RestMethod -UseBasicParsing `
            -Uri "https://api.github.com/repos/$repo/releases/tags/$tag" `
            -Headers @{ "User-Agent" = "hvolt-installer" }
        $entry = $rel.assets | Where-Object { $_.name -eq $Name } | Select-Object -First 1
        if (-not $entry) { throw "asset $Name not found in release $tag" }
        Invoke-WebRequest -UseBasicParsing -Headers @{ Accept = "application/octet-stream"; "User-Agent" = "hvolt-installer" } `
            -Uri "https://api.github.com/repos/$repo/releases/assets/$($entry.id)" -OutFile $Out
    }

    Write-Host "hvolt: fetching $asset"
    Fetch-Asset $asset (Join-Path $tmp $asset)
    Fetch-Asset "SHA256SUMS.txt" (Join-Path $tmp "SHA256SUMS.txt")

    $expected = (Get-Content (Join-Path $tmp "SHA256SUMS.txt") |
        Where-Object { (($_ -split '\s+')[1] -split '/')[-1] -eq $asset } |
        ForEach-Object { ($_ -split '\s+')[0] })
    if (-not $expected) { throw "$asset is not listed in SHA256SUMS.txt" }

    $actual = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $tmp $asset)).Hash.ToLower()
    if ($actual -ne $expected.ToLower()) { throw "checksum mismatch for $asset" }

    Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Force -Path $Prefix | Out-Null
    Copy-Item (Join-Path $tmp "hvolt.exe") (Join-Path $Prefix "hvolt.exe") -Force

    Write-Host "hvolt: installed $(& (Join-Path $Prefix 'hvolt.exe') --version) to $Prefix"
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$Prefix*") {
        Write-Host "hvolt: add $Prefix to your PATH to run it by name"
    }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
