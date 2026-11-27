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
if ($Provider -eq 'claude' -and $promptText.Contains('DAEMON_TURN_FIXTURE')) {
    Add-Content -LiteralPath (Join-Path (Get-Location).Path 'daemon-effect.txt') -Value 'once'
    Emit @{type='assistant';message=@{content=@(@{type='tool_use';id='daemon-write';name='Write';input=@{file_path='daemon-effect.txt'}})}}
    Emit @{type='stream_event';event=@{delta=@{text='DAEMON_PARTIAL'}}}
    $ownedChild = Start-Process powershell.exe -ArgumentList '-NoProfile -NonInteractive -Command "Start-Sleep -Seconds 60"' -WindowStyle Hidden -PassThru
    Set-Content -LiteralPath (Join-Path (Get-Location).Path 'daemon-owned-pids.json') -Value (ConvertTo-Json -Compress @($PID, $ownedChild.Id))
    Start-Sleep -Seconds 12
    Emit @{type='result';is_error=$false;result='DAEMON_PARTIAL completed'}
    exit 0
}
if ($promptText.Contains('SLOW_FIXTURE')) { Start-Sleep -Seconds 60 }
if ($Provider -eq 'claude' -and $promptText.Contains('CLAUDE_ACCOUNT_GATEWAY_FIXTURE')) {
    $gateway = Get-Content -LiteralPath (Join-Path (Get-Location).Path 'native-owner.json') -Raw | ConvertFrom-Json
    & node (Join-Path $PSScriptRoot 'native-account-client.mjs') $gateway.executable $gateway.profile (Get-Location).Path
    if ($LASTEXITCODE -ne 0) {throw 'Account gateway fixture failed'}
    Emit @{type='result';is_error=$false;result='NATIVE_ACCOUNT_GATEWAY_OK'}
    exit 0
}
if ($Provider -eq 'claude' -and $promptText.Contains('CLAUDE_ELICIT_FIXTURE')) {
    Emit @{type='control_request';request_id='claude-consent';request=@{subtype='elicitation';mcp_server_name='Owner connector';message='Choose owner service scope';mode='form';requested_schema=@{type='object';properties=@{count=@{type='integer';minimum=1;maximum=4}};required=@('count')}}}
    $answer=[Console]::In.ReadLine() | ConvertFrom-Json
    if ($answer.response.response.action -ne 'accept' -or $answer.response.response.content.count -ne 3) {throw 'Expected typed Claude connector consent'}
    Emit @{type='result';is_error=$false;result='CONNECTOR_WIRE_OK'}
    exit 0
}
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
$autopilotContinuation = $promptText.Contains('Continue the current user request from this native session.')
if ($Provider -eq 'claude' -and $promptText.Contains('AUTOPILOT_FIXTURE_HANDOFF')) {
    Emit @{type='result';is_error=$true;result="You've hit your usage limit. Resets in 30 minutes."}
    exit 0
}
if ($Provider -eq 'gpt' -and $AppServer -and (Test-Path -LiteralPath 'auto-quota-project.txt')) {
    $initial = $promptText.Contains('AUTO_QUOTA_RESTART_FIXTURE')
    $resume = $env:LUMEN_FIXTURE_RESUME -eq 'true'
    if ($initial -eq $resume) {throw 'Expected the original request only on thread/start'}
    if (!$promptText.Contains('Lumen Autopilot completion check.')) {throw 'Expected Autopilot to confirm completion after quota reset'}
    Add-Content -LiteralPath 'quota-attempts.txt' -Value $(if ($resume) {'resume'} else {'initial'})
    if ($resume) {
        if ((Get-Content -LiteralPath 'quota-effect.txt').Count -ne 1) {throw 'Earlier edit is missing or duplicated'}
        Set-Content -LiteralPath 'quota-finished.txt' -Value 'finished'
        Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='quota-finish';type='agentMessage';text='Finished after reset. <!-- lumen-autopilot: done -->'}}}
        $turn = @{id='fixture-turn';status='completed'}
    } else {
        Add-Content -LiteralPath 'quota-effect.txt' -Value 'once'
        Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='quota-write';type='fileChange';changes=@(@{path='quota-effect.txt';kind='add'})}}}
        Emit @{method='item/agentMessage/delta';params=@{threadId='fixture-thread';turnId='fixture-turn';itemId='quota-partial';delta='Recorded the first edit.'}}
        $turn = @{id='fixture-turn';status='failed';error=@{message='Rate limit reached. Resets in 12 seconds.'}}
    }
    Emit @{method='turn/completed';params=@{threadId='fixture-thread';turn=$turn}}
    exit 0
}
if ($Provider -eq 'gpt' -and $AppServer -and ($promptText.Contains('AUTOPILOT_FIXTURE_') -or $autopilotContinuation)) {
    $hasCheck = $promptText.Contains('Lumen Autopilot completion check.')
    if ($autopilotContinuation) {
        if ($env:LUMEN_FIXTURE_RESUME -ne 'true' -or !$hasCheck -or $promptText.Contains('AUTOPILOT_FIXTURE_')) {throw 'Expected a native continuation without request replay'}
        Add-Content -LiteralPath 'autopilot-attempts.txt' -Value 'continue'
        $reply = "Ho ancora lavoro da fare.`n<!-- lumen-autopilot: continue -->"
        if (Test-Path -LiteralPath 'autopilot-can-finish.txt') {
            Add-Content -LiteralPath 'autopilot-result.txt' -Value 'written once'
            $reply = "Modifica eseguita.`n<!-- lumen-autopilot: done -->"
        }
        if (Test-Path -LiteralPath 'autopilot-handoff-start.txt') {
            if ((Get-Content -LiteralPath 'autopilot-handoff-start.txt').Count -ne 1) {throw 'Handoff edits were replayed'}
            $step = (Get-Content -LiteralPath 'autopilot-attempts.txt').Count - 1
            if ($step -eq 4) {
                Add-Content -LiteralPath 'autopilot-result.txt' -Value 'written once'
                $reply = 'Handoff completed. <!-- lumen-autopilot: done -->'
            } else {
                $file = "autopilot-handoff-step-$step.txt"
                Add-Content -LiteralPath $file -Value "step $step"
                Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id="edit-$step";type='fileChange';status='completed';changes=@(@{path=$file;kind=@{type='add'};diff="+step $step"})}}}
                $reply = "Step $step is complete. Continue with the next check. <!-- lumen-autopilot: continue -->"
            }
        }
        if (Test-Path -LiteralPath 'autopilot-unconfirmed.txt') { $reply = 'Completion still unconfirmed.' }
    } elseif ($promptText.Contains('AUTOPILOT_FIXTURE_HANDOFF')) {
        if (!$hasCheck -or $env:LUMEN_FIXTURE_RESUME -eq 'true') {throw 'Expected a new Codex thread after the Claude limit'}
        Add-Content -LiteralPath 'autopilot-attempts.txt' -Value 'autopilot'
        Add-Content -LiteralPath 'autopilot-handoff-start.txt' -Value 'first step'
        Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='first-edit';type='fileChange';status='completed';changes=@(@{path='autopilot-handoff-start.txt';kind=@{type='add'};diff='+first step'})}}}
        $reply = 'The first edit is complete. Make the second edit next. <!-- lumen-autopilot: continue -->'
    } elseif ($promptText.Contains('AUTOPILOT_FIXTURE_QUESTION')) {
        $reply = 'Risposta alla domanda. <!-- lumen-autopilot: done -->'
    } elseif ($promptText.Contains('AUTOPILOT_FIXTURE_BLOCKED')) {
        $reply = "Il servizio richiesto non risponde.`n<!-- lumen-autopilot: blocked -->"
    } elseif ($promptText.Contains('AUTOPILOT_FIXTURE_DECISION')) {
        if (!$promptText.Contains('call lumen_request_user_input')) {throw 'Codex must receive the named question tool instruction'}
        $question = @{id='priority';header='Routing';question='Which priority should Auto use?';multiSelect=$false;options=@(@{label='Quality';description='Prefer stronger answers'},@{label='Speed';description='Prefer a faster response'})}
        Emit @{id='fixture-decision';method='item/tool/call';params=@{threadId='fixture-thread';turnId='fixture-turn';tool='lumen_request_user_input';arguments=@{questions=@($question)}}}
        $replyFrame = [Console]::In.ReadLine() | ConvertFrom-Json
        $answer = $replyFrame.result.contentItems[0].text | ConvertFrom-Json
        if (!$replyFrame.result.success -or $answer.answers.priority[0] -ne 'Quality') {throw 'Expected the priority chosen in Lumen'}
        $reply = 'Priority confirmed. <!-- lumen-autopilot: done -->'
    } else {
        $access = if ($hasCheck) { 'autopilot' } else { 'restricted' }
        Add-Content -LiteralPath 'autopilot-attempts.txt' -Value $access
        if ($promptText.Contains('AUTOPILOT_FIXTURE_UNCONFIRMED')) { Set-Content -LiteralPath 'autopilot-unconfirmed.txt' -Value '' }
        if ($promptText.Contains('AUTOPILOT_FIXTURE_LOOP')) { Set-Content -LiteralPath 'autopilot-loop.txt' -Value '' }
        if ($promptText.Contains('AUTOPILOT_FIXTURE_LIMIT')) { Set-Content -LiteralPath 'autopilot-limit.txt' -Value '' }
        if ($hasCheck -and $promptText.Contains('AUTOPILOT_FIXTURE_EDIT')) {
            Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='autopilot-read';type='commandExecution';command='read a source file';aggregatedOutput='source file read';exitCode=0;status='completed'}}}
        }
        $reply = 'Ho individuato il file, ma il turno si è interrotto prima della modifica.'
    }
    if (Test-Path -LiteralPath 'autopilot-loop.txt') {
        $step = (Get-Content -LiteralPath 'autopilot-attempts.txt').Count - 1
        $check = @('check A', 'check B')[$step % 2]
        Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id="check-$step";type='commandExecution';command=$check;exitCode=0;status='completed';aggregatedOutput="Passed in $step milliseconds"}}}
        $reply = "Repeated verification $step. <!-- lumen-autopilot: continue -->"
    }
    if (Test-Path -LiteralPath 'autopilot-limit.txt') {
        $step = (Get-Content -LiteralPath 'autopilot-attempts.txt').Count - 1
        Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id="new-$step";type='commandExecution';command="new check $step";exitCode=0;status='completed';aggregatedOutput='passed'}}}
        $reply = "New check $step passed. <!-- lumen-autopilot: continue -->"
    }
    Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='autopilot-message';type='agentMessage';text=$reply}}}
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
$resetMarker = if ($Provider -eq 'claude' -and $env:LUMEN_FIXTURE_SESSION) { Join-Path $env:CLAUDE_CONFIG_DIR ('quota-reset-' + $env:LUMEN_FIXTURE_SESSION + '.mode') } else { $null }
if ($Provider -eq 'claude' -and ($promptText.Contains('QUOTA_RESET_FIXTURE') -or ($resetMarker -and (Test-Path -LiteralPath $resetMarker)))) {
    Add-Content -LiteralPath 'quota-reset-attempts.txt' -Value $(if($env:LUMEN_FIXTURE_RESUME -eq 'true') {'resume'} else {'initial'})
    if ($env:LUMEN_FIXTURE_RESUME -eq 'true') {
        if ((Get-Content -LiteralPath $resetMarker -Raw).Contains('missing')) { Emit @{type='result';is_error=$true;result='session not found'} }
        else { Emit @{type='result';is_error=$false;result='RESUME_AFTER_QUOTA_OK'} }
    } else {
        Set-Content -LiteralPath $resetMarker -Value $(if($promptText.Contains('QUOTA_RESET_MISSING')){'missing'}else{'normal'})
        Add-Content -LiteralPath 'quota-reset-effect.txt' -Value 'once'
        Emit @{type='assistant';message=@{content=@(@{type='tool_use';id='quota-once';name='Write';input=@{file_path='quota-reset-effect.txt'}})}}
        Emit @{type='result';is_error=$true;result=$(if($promptText.Contains('QUOTA_RESET_CANCEL')){"You've hit your usage limit. Resets in 5 seconds."}else{"You've hit your 5-hour usage limit. Resets in 1 second."})}
    }
    exit 0
}
if ($promptText.Contains('FEATURE_TAGS')) {
    if (!$promptText.Contains('<lumen-remember>') -or !$promptText.Contains('<lumen-routine')) { Emit @{type='result';is_error=$false;result='HINTS_MISSING'}; exit 0 }
    Emit @{type='result';is_error=$false;result="Dependencies checked.`n<lumen-remember>Use pnpm for installs</lumen-remember>`n<lumen-remember scope=`"global`">Reply in Italian</lumen-remember>`n<lumen-routine every=`"30m`" until=`"2027-08-30`">Update this project's dependencies</lumen-routine>"}
    exit 0
}
if ($promptText.Contains('QUOTA_CHAIN')) {
    Add-Content -LiteralPath 'quota-chain.txt' -Value $Provider
    $errorText = 'Your workspace is out of credits. Ask your workspace owner to refill in order to continue.'
    if ($Provider -eq 'claude') { Emit @{type='result';is_error=$true;result=$errorText} }
    else {
        Emit @{method='error';params=@{error=@{message=$errorText}}}
        Emit @{method='turn/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';turn=@{id='fixture-turn';status='failed';error=@{message=$errorText}}}}
    }
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
if ($promptText.Contains('LARGE_TOOL_FIXTURE') -and $Provider -eq 'claude') {
    # A tool step over the 16 KiB wire limit: the client receives it shortened, with a detail digest.
    Emit @{type='assistant';message=@{content=@(@{type='tool_use';id='large-read';name='Read';input=@{file_path='large.txt';note=(('fixture line ' * 6) + "`n") * 600 + 'LARGE_TOOL_END'}})}}
    Emit @{type='result';is_error=$false;result='Large tool fixture done.'}
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
# Attention fixtures for the client (M4): one Gatekeeper request or one secret question, then a
# reply that says what the CLI received. Nothing runs: the command text is only displayed.
if ($Provider -eq 'claude' -and $promptText.Contains('APPROVAL_FIXTURE')) {
    Emit @{type='control_request';request_id='fixture-approval';request=@{subtype='can_use_tool';tool_name='Bash';tool_use_id='toolu_fixture';input=@{command='fixture-only: git status --short';description='Show the working tree status'}}}
    $answer = [Console]::In.ReadLine() | ConvertFrom-Json
    $behavior = "$($answer.response.response.behavior)".ToUpper()
    Emit @{type='result';is_error=$false;result="APPROVAL_FIXTURE_$behavior"}
    exit 0
}
if ($Provider -eq 'gpt' -and $AppServer -and $promptText.Contains('APPROVAL_FIXTURE')) {
    Emit @{id='fixture-approval';method='item/commandExecution/requestApproval';params=@{threadId='fixture-thread';turnId='fixture-turn';itemId='approval-item';command='fixture-only: git status --short';cwd=(Get-Location).Path;reason='Show the working tree status'}}
    $answer = [Console]::In.ReadLine() | ConvertFrom-Json
    $decision = "$($answer.result.decision)".ToUpper()
    Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='approval-answer';type='agentMessage';text="APPROVAL_FIXTURE_$decision"}}}
    Emit @{method='turn/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';turn=@{id='fixture-turn';status=$(if ($decision -eq 'CANCEL') {'interrupted'} else {'completed'})}}}
    exit 0
}
if ($Provider -eq 'gpt' -and $AppServer -and $promptText.Contains('SECRET_ASK_FIXTURE')) {
    Emit @{id='fixture-secret';method='item/tool/requestUserInput';params=@{threadId='fixture-thread';turnId='fixture-turn';itemId='secret-item';isBlocking=$true;questions=@(@{id='token';header='Token';question='Paste the fixture token';isSecret=$true;options=$null})}}
    $answer = [Console]::In.ReadLine() | ConvertFrom-Json
    $state = if ($answer.result.answers.token.answers[0] -eq 'fixture-secret-value') {'SECRET_QUESTION_RECEIVED'} else {'SECRET_QUESTION_WRONG'}
    Emit @{method='item/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';item=@{id='secret-answer';type='agentMessage';text=$state}}}
    Emit @{method='turn/completed';params=@{threadId='fixture-thread';turnId='fixture-turn';turn=@{id='fixture-turn';status='completed'}}}
    exit 0
}
$reply = "This is an **offline UI fixture**, not a live AI response.`n`n### A small, thoughtful plan`n`n- Keep the conversation clear.`n- Review the changes before applying them.`n- Make room for the next idea.`n`n| Step | State |`n| --- | --- |`n| Understand | Complete |`n| Review | Complete |`n`n" + '```typescript' + "`nconst clarity = true;`n" + '```'
if (Test-Path -LiteralPath 'verification-probe.cjs') {
    if ($Provider -eq 'claude' -and $promptText.Contains('Validation found concrete errors after your changes.')) {
        if (!$promptText.Contains('EVIDENCE_FIXTURE_ERROR') -or $promptText.Contains('EVIDENCE_FIXTURE_NOISE')) {throw 'Expected compact failure evidence'}
        Set-Content -LiteralPath 'verification-target.txt' -Value 'fixed'
        $reply = 'Corrected the fixture.'
    } elseif ($Provider -eq 'claude' -and $promptText.Contains('VERIFY_AND_REVIEW_FIXTURE')) {
        Set-Content -LiteralPath 'verification-target.txt' -Value 'broken'
        $reply = 'Edited the fixture.'
    } elseif ($Provider -eq 'gpt' -and $promptText.Contains('You are auditing')) {
        if (!$promptText.Contains('Exit code: 0') -or $promptText.Contains('EVIDENCE_FIXTURE_NOISE') -or $promptText.Contains('EVIDENCE_FIXTURE_ERROR')) {throw 'Expected only current passing evidence in review'}
    } elseif ($Provider -eq 'gpt' -and $promptText.Contains('VERIFY_HANDOFF_FIXTURE')) {
        if (!$promptText.Contains('Exit code: 0') -or $promptText.Contains('EVIDENCE_FIXTURE_NOISE') -or $promptText.Contains('EVIDENCE_FIXTURE_ERROR')) {throw 'Expected only current passing evidence in handoff'}
        $reply = 'VERIFICATION_HANDOFF_OK'
    }
}
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
    if ($promptText.Contains('DAEMON_ORCHESTRA_FIXTURE')) { Emit @{type='stream_event';event=@{delta=@{text='ORCHESTRA_WORKER_PARTIAL'}}}; Start-Sleep -Seconds 8 }
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
