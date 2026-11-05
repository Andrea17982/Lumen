param([string]$Provider)
# Offline test fixture. Never authenticates or calls a provider.
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$promptText = [Console]::In.ReadToEnd()
if ($promptText.Contains('SLOW_FIXTURE')) { Start-Sleep -Seconds 60 }
$reply = "This is an **offline UI fixture**, not a live AI response.`n`n### A small, thoughtful plan`n`n- Keep the conversation clear.`n- Review the changes before applying them.`n- Make room for the next idea.`n`n| Step | State |`n| --- | --- |`n| Understand | Complete |`n| Review | Complete |`n`n" + '```typescript' + "`nconst clarity = true;`n" + '```'
function Emit($value) { ConvertTo-Json -InputObject $value -Depth 10 -Compress | Write-Output }
if ($Provider -eq 'gpt') {
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
