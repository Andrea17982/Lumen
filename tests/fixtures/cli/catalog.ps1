param([string]$Provider)
# Official-protocol-shaped offline metadata. No inference or authentication.
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
function Emit($value) { ConvertTo-Json -InputObject $value -Depth 15 -Compress | Write-Output }
while ($null -ne ($line=[Console]::In.ReadLine())) {
    $request=$line | ConvertFrom-Json
    if ($Provider -eq 'gpt') {
        if ($request.method -eq 'initialize') { Emit @{id=$request.id;result=@{userAgent='offline-fixture'}} }
        if ($request.method -eq 'model/list') {
            Emit @{id=$request.id;result=@{data=@(
                @{id='gpt-fixture';model='gpt-fixture';displayName='GPT fixture';isDefault=$true;defaultReasoningEffort='medium';supportedReasoningEfforts=@(@{reasoningEffort='low'},@{reasoningEffort='medium'},@{reasoningEffort='high'},@{reasoningEffort='xhigh'},@{reasoningEffort='ultra'})},
                @{id='gpt-fast-fixture';model='gpt-fast-fixture';displayName='GPT fast fixture';defaultReasoningEffort='low';supportedReasoningEfforts=@(@{reasoningEffort='low'},@{reasoningEffort='medium'})}
            );nextCursor=$null}}
        }
    } elseif ($request.request.subtype -eq 'initialize') {
        Emit @{type='control_response';response=@{subtype='success';request_id=$request.request_id;response=@{models=@(
            @{value='opus';displayName='Opus fixture';supportedEffortLevels=@('low','medium','high','xhigh','max')},
            @{value='sonnet';displayName='Sonnet fixture';supportedEffortLevels=@('low','medium','high')}
        )}}}
    }
}
