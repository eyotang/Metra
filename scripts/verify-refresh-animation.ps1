$ErrorActionPreference = "Stop"

$stylesPath = Join-Path $PSScriptRoot "..\src\styles.css"
$styles = Get-Content -Raw -Encoding UTF8 $stylesPath

$unsafeSelectors = [System.Collections.Generic.List[string]]::new()
$rulePattern = '(?ms)(?<selectors>[^{}]+)\{(?<body>[^{}]*\banimation(?:-name)?\s*:\s*[^;}]*\bspin\b[^{}]*)\}'

foreach ($match in [regex]::Matches($styles, $rulePattern)) {
    foreach ($selector in $match.Groups['selectors'].Value.Split(',')) {
        $candidate = $selector.Trim()
        $isDedicatedVisual = $candidate -match '(::before|::after|(^|[\s>+~])i(?:[.:#\[]|$)|\.spinner(?:[.:#\[]|$))'
        if (-not $isDedicatedVisual) {
            $unsafeSelectors.Add($candidate)
        }
    }
}

if ($unsafeSelectors.Count -gt 0) {
    throw "Spin animation targets non-icon nodes: $($unsafeSelectors -join ', ')"
}

Write-Output "Refresh animation contract passed: spin only targets icons or pseudo-elements."
