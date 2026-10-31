$ErrorActionPreference='Stop'
$arcProcesses=@(Get-CimInstance Win32_Process|Where-Object {$_.Name -in @('cargo.exe','cargo-nextest.exe','rustc.exe','link.exe')})
$arcLive=@($arcProcesses|Where-Object {$_.CommandLine -match '(?i)D:[\\/]git[\\/]arcweft(?:[\\/ ]|$)|--crate-name[ =]+arcweft|(?:--package|-p)[ =]+arcweft'})
if($arcLive.Count){throw ('Live Arcweft build process IDs: '+($arcLive.ProcessId -join ','))}
