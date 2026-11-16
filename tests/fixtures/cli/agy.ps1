# Offline protocol fixture. No login or provider requests.
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
if ($args -contains 'models') {
    Write-Output '[{"id":"gemini-3.8-flash-low","displayName":"Gemini Flash","supportedEffortLevels":["low","high"]},{"id":"claude-opus","displayName":"Retired third-party model"}]'
    exit 0
}
if ($args -contains '/usage') {
    Write-Output "Gemini Models`tFive Hour Limit Remaining`t99%`t2027-01-01T00:00:00Z"
    exit 0
}
$line = [Console]::ReadLine()
if (!$line) { exit 2 }
$request = $line | ConvertFrom-Json
if ($request.message.content -like '*AG_CORPUS_SLOW*') {
    $child = Start-Process -FilePath 'node.exe' -ArgumentList '-e','setTimeout(()=>{},60000)' -WindowStyle Hidden -PassThru
    Set-Content -LiteralPath 'agy-owned-pids.json' -Value (ConvertTo-Json -InputObject @($PID,$child.Id) -Compress)
    Write-Output '{"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"Partial Antigravity work retained"}}'
    Start-Sleep -Seconds 60
    exit 0
}
if ($request.message.content -like '*AG_CORPUS_ERROR*' -or $request.message.content -like '*AG_CORPUS_QUOTA*') {
    Write-Output '{"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"Partial Antigravity work retained"}}'
    $message = if ($request.message.content -like '*AG_CORPUS_QUOTA*') {'Quota exhausted. Try again in 1 second.'} else {'Fixture terminal failure'}
    ConvertTo-Json -InputObject @{event='result';result=@{status='ERROR';error=$message}} -Depth 8 -Compress | Write-Output
    exit 0
}
if ($request.message.content -like '*QUOTA_CHAIN*') { Add-Content -LiteralPath 'quota-chain.txt' -Value 'antigravity' }
Write-Output '{"event":"init","conversation_id":"offline-agy"}'
Write-Output '{"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"GOOGLE_REPLY_OK","usage":{"input_tokens":42000}}}'
Write-Output '{"event":"result","result":{"status":"SUCCESS","response":"GOOGLE_REPLY_OK"}}'
# The real stream-input connection also remains open for subsequent prompts.
$null = [Console]::ReadLine()
