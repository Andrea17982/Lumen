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
if ($promptText.Contains('SLOW_FIXTURE')) { Start-Sleep -Seconds 60 }
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
