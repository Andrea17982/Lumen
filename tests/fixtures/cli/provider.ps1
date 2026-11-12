param([string]$Provider, [string]$PromptText = '', [switch]$AppServer)
# Offline test fixture. Never authenticates or calls a provider.
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
function Emit($value) { ConvertTo-Json -InputObject $value -Depth 15 -Compress | Write-Output }
if (!$PromptText) {
    if ($Provider -eq 'claude') {
        $firstLine = [Console]::In.ReadLine()
        $initialize = $null
        if ($firstLine.StartsWith('{')) { try { $initialize = $firstLine | ConvertFrom-Json } catch {} }
        if ($initialize.type -eq 'control_request') {
            Emit @{type='control_response';response=@{subtype='success';request_id=$initialize.request_id;response=@{}}}
            $user = [Console]::In.ReadLine() | ConvertFrom-Json
            $promptText = $user.message.content
            Emit @{type='system';subtype='init';session_id=$env:LUMEN_FIXTURE_SESSION}
            Emit @{type='rate_limit_event';rate_limit_info=@{unifiedWindows=@{five_hour=@{utilization=0.20;resetsAt=[DateTimeOffset]::UtcNow.AddHours(1).ToUnixTimeSeconds()};seven_day=@{utilization=0.90;resetsAt=[DateTimeOffset]::UtcNow.AddMinutes(15).ToUnixTimeSeconds()}}}}
        } else { $promptText = $firstLine + "`n" + [Console]::In.ReadToEnd() }
    } else { $promptText = [Console]::In.ReadToEnd() }
}
if ($Provider -eq 'claude' -and $promptText.Contains('PROMPT_BUDGET_FIXTURE')) {
    Add-Content -LiteralPath (Join-Path $env:CLAUDE_CONFIG_DIR 'context-payloads.jsonl') -Value (ConvertTo-Json -Compress -Depth 4 @{prompt=$promptText;workspace=(Get-Location).Path;resume=$env:LUMEN_FIXTURE_RESUME}) -Encoding utf8
}
if ($promptText.Contains('SLOW_FIXTURE')) { Start-Sleep -Seconds 60 }
if ($Provider -eq 'claude' -and $promptText.Contains('CLAUDE_MCP_FIXTURE')) {
    Emit @{type='system';subtype='init';session_id=$env:LUMEN_FIXTURE_SESSION;mcp_servers=@(@{name='offline-github';status='connected';url='https://example.com?secret=never-store-this'})}
    Emit @{type='control_request';request_id='mcp-review';request=@{subtype='can_use_tool';tool_name='mcp__github__create_issue';input=@{repository='offline';title='Review once'}}}
    $answer = [Console]::In.ReadLine() | ConvertFrom-Json
    if ($answer.response.response.behavior -ne 'allow' -or $answer.response.response.updatedInput.repository -ne 'offline') {throw 'MCP approval did not preserve native input'}
    Emit @{type='result';is_error=$false;result='CONNECTOR_WIRE_OK'}
    exit 0
}
if ($Provider -eq 'gpt' -and $promptText.Contains('CONNECTOR_FIXTURE')) {
    $params = @{threadId='fixture-thread';turnId='fixture-turn';serverName='Offline GitHub';message='Allow access for this request?';mode='form';requestedSchema=@{type='object';properties=@{count=@{type='integer';minimum=1;maximum=4};note=@{type='string'}};required=@('count')}}
    if ($promptText.Contains('URL')) {
        $params = @{threadId='fixture-thread';turnId='fixture-turn';serverName='Offline GitHub';message='Authorize https://example.com/auth?secret=never-store-this';mode='url';url='https://example.com/auth?secret=never-store-this';elicitationId='offline-auth'}
    }
    if ($promptText.Contains('UNSUPPORTED')) {$params.requestedSchema.properties.count.type='array'}
    Emit @{id='connector-request';method='mcpServer/elicitation/request';params=$params}
    if ($promptText.Contains('EXPIRE')) {
        Start-Sleep -Seconds 3
        Emit @{method='serverRequest/resolved';params=@{threadId='fixture-thread';requestId='connector-request'}}
    } else {
        $answer = [Console]::In.ReadLine() | ConvertFrom-Json
        $expected = if ($promptText.Contains('UNSUPPORTED')) {'cancel'} elseif ($promptText.Contains('DECLINE')) {'decline'} else {'accept'}
        if ($answer.id -ne 'connector-request' -or $answer.result.action -ne $expected) {throw 'Connector response has the wrong wire shape or action'}
        if ($expected -eq 'accept' -and !$promptText.Contains('URL') -and $answer.result.content.count -ne 3) {throw 'Expected a typed integer connector answer'}
        if ($expected -ne 'accept' -and $null -ne $answer.result.content) {throw 'Declined connector must have null content'}
    }
    Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='connector-answer';type='agentMessage';text='CONNECTOR_WIRE_OK'}}}
    Emit @{method='turn/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';turn=@{id='fixture-turn';status='completed'}}}
    exit 0
}
if ($Provider -eq 'claude' -and $promptText.Contains('You are one agent in a team') -and $promptText.Contains('ORCHESTRA_CONTINUITY_FIXTURE')) {
    Add-Content -LiteralPath (Join-Path $env:CLAUDE_CONFIG_DIR 'worker-continuity-attempts.txt') -Value 'attempt'
    if ($promptText.Contains('WORKER_CANCEL')) {
        $ownedChild = Start-Process powershell.exe -ArgumentList '-NoProfile -NonInteractive -Command "Start-Sleep -Seconds 60"' -WindowStyle Hidden -PassThru
        Set-Content -LiteralPath (Join-Path $env:CLAUDE_CONFIG_DIR 'worker-cancel-pids.json') -Value (ConvertTo-Json -Compress @($PID, $ownedChild.Id))
        Emit @{type='stream_event';event=@{delta=@{text='Worker waiting with owned child.'}}}
        Start-Sleep -Seconds 60
    } elseif ($promptText.Contains('WORKER_QUOTA_AFTER_EFFECT')) {
        Set-Content -LiteralPath (Join-Path (Get-Location).Path 'owned-worker.txt') -Value 'owned worktree effect'
        Emit @{type='assistant';message=@{content=@(@{type='tool_use';id='worker-write';name='Write';input=@{file_path='owned-worker.txt'}})}}
        Emit @{type='stream_event';event=@{delta=@{text='Preserve worker partial output.'}}}
        Emit @{type='result';is_error=$true;result="You've hit your usage limit. Resets in 30 minutes."}
    } else { Emit @{type='result';is_error=$false;result=$(if ($promptText.Contains('APPROVED_SCOPE_MARKER')) { 'WORKER_SCOPE_PRESENT' } else { 'WORKER_SCOPE_MISSING' })} }
    exit 0
}
if ($promptText.Contains('QUOTA_GUARD_')) {
    $case = if ($promptText.Contains('QUOTA_GUARD_NESTED')) { 'nested' } elseif ($promptText.Contains('QUOTA_GUARD_TOOL')) { 'tool' } elseif ($promptText.Contains('QUOTA_GUARD_PARTIAL')) { 'partial' } else { 'safe' }
    Add-Content -LiteralPath (Join-Path (Get-Location).Path "quota-attempt-$case.txt") -Value $Provider
    if ($case -eq 'nested' -and $Provider -eq 'claude' -and $env:LUMEN_FIXTURE_RESUME -eq 'true') {
        Emit @{type='result';is_error=$true;result='session not found'}
        exit 0
    }
    if ($case -eq 'tool') { Add-Content -LiteralPath (Join-Path (Get-Location).Path 'quota-effect.txt') -Value $Provider }
    if ($case -eq 'nested') { Add-Content -LiteralPath (Join-Path (Get-Location).Path 'quota-effect-nested.txt') -Value $Provider }
    if ($Provider -eq 'claude') {
        if ($case -in @('tool', 'nested')) { Emit @{type='assistant';message=@{content=@(@{type='tool_use';id='quota-write';name='Write';input=@{file_path='quota-effect.txt';content='owned effect'}})}} }
        if ($case -eq 'partial') { Emit @{type='stream_event';event=@{delta=@{type='text_delta';text='Preserve this partial result.'}}} }
        Emit @{type='result';is_error=$true;result="You've hit your usage limit. Resets in 30 minutes."}
    } else {
        Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='quota-answer';type='agentMessage';text='QUOTA_FALLBACK_RECOVERED'}}}
        Emit @{method='turn/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';turn=@{id='fixture-turn';status='completed'}}}
    }
    exit 0
}
if ($promptText.Contains('REPLAY_TOOL_FIXTURE') -or $promptText.Contains('REPLAY_MISSING_FIXTURE')) {
    $tool = $promptText.Contains('REPLAY_TOOL_FIXTURE')
    $counter = if ($tool) { "replay-tool-$Provider.txt" } else { "replay-missing-$Provider.txt" }
    Add-Content -LiteralPath (Join-Path (Get-Location).Path $counter) -Value 'invoked'
    $fail = $tool -or $env:LUMEN_FIXTURE_RESUME -eq 'true'
    if ($Provider -eq 'claude') {
        if ($tool) { Emit @{type='assistant';message=@{content=@(@{type='tool_use';id='replay-write';name='Write';input=@{file_path=$counter;content='owned fixture effect'}})}} }
        Emit @{type='result';is_error=$fail;result=$(if ($fail) { 'session not found' } else { 'REPLAY_RECOVERED' })}
    } else {
        if ($tool) { Emit @{method='item/started';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='replay-write';type='commandExecution';command='fixture-owned write'}}} }
        if (!$fail) { Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='replay-answer';type='agentMessage';text='REPLAY_RECOVERED'}}} }
        Emit @{method='turn/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';turn=@{id='fixture-turn';status=$(if ($fail) { 'failed' } else { 'completed' });error=@{message='thread not found'}}}}
    }
    exit 0
}
if ($promptText.Contains('OBS_PROMISE_FIXTURE') -and $Provider -eq 'claude') {
    Emit @{type='result';is_error=$false;result='I will implement the requested changes.'}
    exit 0
}
if ($promptText.Contains('OBS_ERROR_FIXTURE') -and $Provider -eq 'claude') {
    Emit @{type='result';is_error=$true;result='Controlled offline provider failure.'}
    exit 1
}
if ($promptText.Contains('OBS_TOOL_FIXTURE') -and $Provider -eq 'claude') {
    Emit @{type='assistant';message=@{content=@(@{type='tool_use';id='observed-tool';name='Read';input=@{file_path='sample.ts'}})}}
    Start-Sleep -Seconds 2
    Emit @{type='user';message=@{content=@(@{type='tool_result';tool_use_id='observed-tool';content='Offline result.'})}}
    Emit @{type='result';is_error=$false;result='The offline tool returned.'}
    exit 0
}
if ($promptText.Contains('QUESTION_FIXTURE')) {
    if ($Provider -eq 'claude') {
        Emit @{type='control_request';request_id='fixture-question';request=@{subtype='can_use_tool';tool_name='AskUserQuestion';input=@{questions=@(@{question='Which implementation should I use?';header='Implementation';multiSelect=$false;options=@(@{label='Native';description='Keep official CLI behavior'},@{label='Custom';description='Use a custom approach'})})}}}
        $answer = [Console]::In.ReadLine() | ConvertFrom-Json
        if ($answer.response.response.updatedInput.answers.'Which implementation should I use?' -ne 'Native') { throw 'Expected the real answer through Claude stdin' }
    } else {
        Emit @{id='fixture-question';method='item/tool/requestUserInput';params=@{threadId='fixture-thread';turnId='fixture-turn';itemId='question-item';isBlocking=$true;questions=@(@{id='implementation';header='Implementation';question='Which implementation should I use?';options=@(@{label='Native';description='Keep official CLI behavior'},@{label='Custom';description='Use a custom approach'})})}}
        $answer = [Console]::In.ReadLine() | ConvertFrom-Json
        if ($answer.result.answers.implementation.answers[0] -ne 'Native') { throw 'Expected the real answer through Codex stdin' }
    }
    Set-Content -LiteralPath (Join-Path (Get-Location).Path "answer-$Provider.txt") -Value 'Native'
}
$reply = "This is an **offline UI fixture**, not a live AI response.`n`n### A small, thoughtful plan`n`n- Keep the conversation clear.`n- Review the changes before applying them.`n- Make room for the next idea.`n`n| Step | State |`n| --- | --- |`n| Understand | Complete |`n| Review | Complete |`n`n" + '```typescript' + "`nconst clarity = true;`n" + '```'
if ($promptText.Contains('QUALITY_CONTEXT_FIXTURE')) {
    $reply = if ($promptText.Contains('Independent counter check: failed')) { 'QUALITY_CONTEXT_PRESENT' } else { 'QUALITY_CONTEXT_ABSENT' }
}
# Orchestra fixtures: questions, a two-part plan, approvals and one file per subtask.
if ($promptText.Contains('Ask only for missing information that materially changes the result.')) {
    $reply = '{"questions":[{"q":"Which file names?","recommended":"orchestra-s1.txt and orchestra-s2.txt"}]}'
} elseif ($promptText.Contains('Plan the work below')) {
    $reply = '{"summary":"Two fixture files","subtasks":[{"id":"a","title":"First file","detail":"Write the first file","files":["orchestra-s1.txt"],"difficulty":"normal","writes":true,"dependsOn":[]},{"id":"b","title":"Second file","detail":"Write the second file","files":["orchestra-s2.txt"],"difficulty":"normal","writes":true,"dependsOn":["a"]}]}'
} elseif ($promptText.Contains('You are auditing')) {
    $reply = 'APPROVED'
}
if ($promptText -match 'Your subtask \((s\d+)\)') {
    $name = "orchestra-$($Matches[1]).txt"
    Set-Content -LiteralPath (Join-Path (Get-Location).Path $name) -Value "by $Provider" -Encoding utf8
    $reply = "Wrote $name"
}
if ($Provider -eq 'ollama') {
    Write-Output "Local fixture reply."
    exit 0
}
$rateOnce = if ($Provider -eq 'claude' -and $env:CLAUDE_CONFIG_DIR) { Join-Path $env:CLAUDE_CONFIG_DIR 'quality-rate-once' } else { $null }
if ($rateOnce -and (Test-Path -LiteralPath $rateOnce)) {
    Remove-Item -LiteralPath $rateOnce
    Emit @{type='result';is_error=$true;result="You've hit your usage limit. Resets in 30 minutes."}
    exit 1
}
if ($promptText.Contains('RATE_FIXTURE') -and $Provider -eq 'claude') {
    Emit @{type='result';is_error=$true;result="You've hit your usage limit. Resets in 30 minutes."}
    exit 1
}
if ($promptText.Contains('WRITE_FIXTURE')) {
    # Writes only inside the working directory the app chose (a temp test workspace or worktree).
    Set-Content -LiteralPath (Join-Path (Get-Location).Path "fixture-$Provider.txt") -Value "written by $Provider" -Encoding utf8
    Set-Content -LiteralPath (Join-Path (Get-Location).Path 'shared.txt') -Value "edited by $Provider" -Encoding utf8
}
if ($Provider -eq 'gpt') {
    if ($AppServer) {
        Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='command-1';type='commandExecution';command='fixture-only: working directory';aggregatedOutput=(Get-Location).Path}}}
        Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='reasoning-1';type='reasoning';summary=@('Offline fixture reasoning summary')}}}
        Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='file-1';type='fileChange';changes=@(@{path='sample.ts';kind='update'})}}}
        Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='message-1';type='agentMessage';text=$reply}}}
        Emit @{method='turn/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';turn=@{id='fixture-turn';status='completed'}}}
        return
    }
    Emit @{type='item.completed';item=@{type='command_execution';command='fixture-only: working directory';aggregated_output=(Get-Location).Path}}
    Emit @{type='item.completed';item=@{type='reasoning';text='Fixture reasoning summary: inspect the sample and form a concise response.'}}
    Emit @{type='item.completed';item=@{type='command_execution';command='fixture-only: inspect sample';aggregated_output='No command was executed in the workspace.'}}
    Emit @{type='item.completed';item=@{type='file_change';changes=@(@{path='sample.ts';kind='update'})}}
    Emit @{type='item.completed';item=@{type='agent_message';text=$reply}}
} else {
    Emit @{type='assistant';message=@{content=@(@{type='tool_use';name='Read';input=@{file_path='sample.ts'}})}}
    $mid = [int]($reply.Length / 2)
    Emit @{type='stream_event';event=@{delta=@{text=$reply.Substring(0,$mid)}}}
    Start-Sleep -Milliseconds 50
    Emit @{type='stream_event';event=@{delta=@{text=$reply.Substring($mid)}}}
    Emit @{type='result';is_error=$false;result=$reply}
}
