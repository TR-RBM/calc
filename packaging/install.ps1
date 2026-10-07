param([switch]$Uninstall)

$ErrorActionPreference = 'Stop'
$target = Join-Path $env:LOCALAPPDATA 'Programs\calc'
$files = @('calc.exe', 'README.md', 'LICENSE', 'LICENSE-CONTENT')
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$entries = @($userPath -split ';' | Where-Object { $_ -ne '' })

if ($Uninstall) {
    foreach ($file in $files) {
        $path = Join-Path $target $file
        if (Test-Path $path) { Remove-Item -LiteralPath $path }
    }
    if ((Test-Path $target) -and -not (Get-ChildItem -LiteralPath $target)) {
        Remove-Item -LiteralPath $target
    }
    $kept = $entries | Where-Object { $_ -ne $target }
    [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User')
    Write-Output "removed $target\calc.exe"
    exit 0
}

New-Item -ItemType Directory -Force -Path $target | Out-Null
foreach ($file in $files) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination $target -Force
}
if ($entries -notcontains $target) {
    [Environment]::SetEnvironmentVariable('Path', (($entries + $target) -join ';'), 'User')
}
Write-Output "installed $target\calc.exe; open a new terminal to have it on the path"
