param([string]$Provider)
# Official-protocol-shaped offline metadata. No inference or authentication.
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
function Emit($value) { ConvertTo-Json -InputObject $value -Depth 15 -Compress | Write-Output }
while ($null -ne ($line=[Console]::In.ReadLine())) {
    $request=$line | ConvertFrom-Json
    if ($Provider -eq 'gpt') {
        if ($request.method -eq 'initialize') { Emit @{id=$request.id;result=@{userAgent='offline-fixture'}} }
        if ($request.method -in @('thread/start','thread/resume')) {
            $env:LUMEN_FIXTURE_RESUME = if ($request.method -eq 'thread/resume') { 'true' } else { 'false' }
            Emit @{id=$request.id;result=@{thread=@{id='fixture-thread'}}}
        }
        if ($request.method -eq 'turn/start') {
            Emit @{id=$request.id;result=@{turn=@{id='fixture-turn'}}}
            & "$PSScriptRoot/provider.ps1" -Provider gpt -PromptText $request.params.input[0].text -AppServer
        }
        if ($request.method -eq 'model/list') {
            Emit @{id=$request.id;result=@{data=@(
                @{id='gpt-fixture';model='gpt-fixture';displayName='GPT Sol fixture';isDefault=$true;defaultReasoningEffort='medium';supportedReasoningEfforts=@(@{reasoningEffort='low'},@{reasoningEffort='medium'},@{reasoningEffort='high'},@{reasoningEffort='xhigh'},@{reasoningEffort='ultra'})},
                @{id='gpt-luna-fixture';model='gpt-luna-fixture';displayName='GPT Luna fixture';defaultReasoningEffort='low';supportedReasoningEfforts=@(@{reasoningEffort='low'},@{reasoningEffort='medium'},@{reasoningEffort='high'})},
                @{id='gpt-terra-fixture';model='gpt-terra-fixture';displayName='GPT Terra fixture';defaultReasoningEffort='low';supportedReasoningEfforts=@(@{reasoningEffort='low'},@{reasoningEffort='medium'},@{reasoningEffort='high'})},
                @{id='gpt-fast-fixture';model='gpt-fast-fixture';displayName='GPT fast fixture';defaultReasoningEffort='low';supportedReasoningEfforts=@(@{reasoningEffort='low'},@{reasoningEffort='medium'})}
            );nextCursor=$null}}
        }
        if ($request.method -eq 'account/rateLimits/read') {
            Emit @{id=$request.id;result=@{rateLimits=@{primary=@{usedPercent=25;resetsAt=[DateTimeOffset]::UtcNow.AddHours(3).ToUnixTimeSeconds();windowDurationMins=300};secondary=@{usedPercent=40;resetsAt=[DateTimeOffset]::UtcNow.AddDays(5).ToUnixTimeSeconds();windowDurationMins=10080}}}}
        }
    } elseif ($request.request.subtype -eq 'initialize') {
        Emit @{type='control_response';response=@{subtype='success';request_id=$request.request_id;response=@{models=@(
            @{value='opus';displayName='Opus fixture';supportedEffortLevels=@('low','medium','high','xhigh','max')},
            @{value='sonnet';displayName='Sonnet fixture';supportedEffortLevels=@('low','medium','high')},
            @{value='haiku';displayName='Haiku fixture';supportedEffortLevels=@('low','medium','high')}
        )}}}
    }
}
