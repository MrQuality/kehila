[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [switch]$Test
)
$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne 'Win32NT') { throw 'This client requires Windows.' }
$output = (Resolve-Path -LiteralPath $OutputDirectory).ProviderPath
$checkout = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..')).TrimEnd('\')
if ($output.TrimEnd('\').Equals($checkout, [StringComparison]::OrdinalIgnoreCase) -or
    $output.StartsWith($checkout + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Build and credential-test output must be outside the checkout.'
}
if (-not (Test-Path -LiteralPath $output -PathType Container)) { throw 'Use a pre-created protected output directory.' }
$ancestor = [IO.DirectoryInfo]::new($output)
while ($null -ne $ancestor) {
    if (($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw 'Output ancestors must not be reparse objects.'
    }
    $ancestor = $ancestor.Parent
}
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
if (-not (Test-Path -LiteralPath $compiler -PathType Leaf)) { throw 'The installed .NET Framework compiler is required.' }
$sources = @('CredentialStore.cs', 'Transport.cs', 'Program.cs') | ForEach-Object { Join-Path $PSScriptRoot $_ }
$references = @('/reference:System.Security.dll', '/reference:System.Net.Http.dll', '/reference:System.Runtime.Serialization.dll')
function Build-Client([string]$entry, [string]$name, [string[]]$files) {
    $target = Join-Path $output $name
    & $compiler /nologo /warnaserror+ /target:exe "/main:Kehila.Auth001.Client.$entry" "/out:$target" @references @files
    if ($LASTEXITCODE -ne 0) { throw 'Client compilation failed.' }
    return $target
}
$null = Build-Client 'Program' 'owner-client.exe' $sources
if ($Test) {
    $store = Build-Client 'CredentialStoreTests' 'credential-store-tests.exe' ($sources + (Join-Path $PSScriptRoot 'CredentialStoreTests.cs'))
    $response = Build-Client 'ResponseTests' 'response-tests.exe' ($sources + (Join-Path $PSScriptRoot 'ResponseTests.cs'))
    $fixture = Join-Path $output ('credential-test-' + [Guid]::NewGuid().ToString('N'))
    & $store $fixture
    if ($LASTEXITCODE -ne 0) { throw 'Credential-store tests failed; retain the external fixture for diagnosis.' }
    & $response
    if ($LASTEXITCODE -ne 0) { throw 'Response tests failed.' }
}
Write-Output 'Windows client built. No requests or trust-store changes were performed.'
