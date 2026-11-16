param([string]$Session)
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
if(Test-Path Env:CURSOR_API_KEY) {exit 9}
$text=[Console]::In.ReadToEnd()
function Emit($v) {ConvertTo-Json -InputObject $v -Compress -Depth 8 | Write-Output}
$resumed=$Session -ne ''
if(!$resumed){$Session=[Guid]::NewGuid().ToString()}
$modeFile=".cursor-fixture-$Session"
if(!$resumed) {Set-Content -LiteralPath $modeFile -Value $text}
$mode=Get-Content -LiteralPath $modeFile -Raw
Add-Content -LiteralPath 'cursor-attempts.jsonl' -Value (ConvertTo-Json -InputObject @{resumed=$resumed;session=$Session;length=$text.Length} -Compress)
Emit @{type='system';subtype='init';session_id=$Session;model='GPT 6.1 Sol';apiKeySource='login'}
if($mode.Contains('CURSOR_SLOW_FIXTURE')) {
 $child=Start-Process powershell.exe -ArgumentList '-NoProfile -NonInteractive -Command "Start-Sleep -Seconds 60"' -WindowStyle Hidden -PassThru
 ConvertTo-Json -InputObject @($PID,$child.Id) | Set-Content -LiteralPath 'cursor-pids.json'
 Emit @{type='assistant';session_id=$Session;message=@{content=@(@{type='text';text='Waiting'})}}
 Start-Sleep -Seconds 60
 exit 0
}
if($mode.Contains('CURSOR_STDERR_QUOTA')) { [Console]::Error.WriteLine('Rate limit reached. Try again in 1 second.'); exit 1 }
if($mode.Contains('CURSOR_TRUNCATED')) {Emit @{type='assistant';session_id=$Session;message=@{content=@(@{type='text';text='Partial retained'})}};exit 0}
if(!$resumed) {
 Emit @{type='tool_call';subtype='started';session_id=$Session;call_id='write-1';tool_call=@{writeToolCall=@{args=@{path='cursor-effects.txt'}}}}
 Add-Content -LiteralPath 'cursor-effects.txt' -Value $Session
 Emit @{type='tool_call';subtype='completed';session_id=$Session;call_id='write-1';tool_call=@{writeToolCall=@{result=@{success=@{}}}}}
}
if(!$resumed -and $mode.Contains('CURSOR_QUOTA_FIXTURE')) {
 Emit @{type='assistant';session_id=$Session;message=@{content=@(@{type='text';text='Existing file work retained'})}}
 Emit @{type='result';subtype='error';is_error=$true;session_id=$Session;result='Quota exhausted. Try again in 1 second.'}
 exit 1
}
$reply=if($resumed){'CURSOR_RESUME_OK'}else{'CURSOR_FIRST_OK'}
Emit @{type='assistant';session_id=$Session;message=@{content=@(@{type='text';text=$reply})}}
Emit @{type='result';subtype='success';is_error=$false;session_id=$Session;result=$reply}
