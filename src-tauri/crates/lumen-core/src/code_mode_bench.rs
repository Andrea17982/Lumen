//! Paired local benchmark. CI uses a scripted loopback model; live inference is opt-in.
use crate::{
    api_tools,
    code_mode::{self, Session},
    models::{AccessMode, Account, Provider},
    ollama_agent::{endpoint, local_model},
    providers::Invocation,
    test_support::Fixture,
};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    time::{Duration, Instant},
};
use tokio::sync::watch;

const FILES: usize = 20;
const WINDOW: u64 = 12_500_000;
const READ: &str = "lumen_read_file";
const WRITE: &str = "lumen_write_file";
const SCAN: &str = "bench_scan_log";
const CHECKS: [&str; 3] = ["bench_test", "bench_lint", "bench_build"];
const CHECK: &str = "bench_check";
// Suite F reuses the Compete corpus checks; solutions are only used by the T0 replay.
const F_TASKS: [(&str, &str); 6] = [
    (crate::compete_corpus::SLUG_CHECK, "module.exports=s=>s.toLowerCase().replace(/[^a-z0-9]+/g,'-').replace(/^-|-$/g,'');"),
    (crate::compete_corpus::DURATION_CHECK, "module.exports=s=>{let t=0;for(const[,n,u]of s.matchAll(/(\\d+)([hm])/g))t+=n*(u==='h'?3600:60);return t;};"),
    (crate::compete_corpus::CLAMP_CHECK, "module.exports=(x,lo,hi)=>Math.min(hi,Math.max(lo,x));"),
    (crate::compete_corpus::DEDUPE_CHECK, "module.exports=xs=>[...new Set(xs)];"),
    (crate::compete_corpus::ROMAN_CHECK, "module.exports=n=>{let s='';for(const[v,r]of[[1000,'M'],[900,'CM'],[500,'D'],[400,'CD'],[100,'C'],[90,'XC'],[50,'L'],[40,'XL'],[10,'X'],[9,'IX'],[5,'V'],[4,'IV'],[1,'I']])while(n>=v){s+=r;n-=v;}return s;};"),
    (crate::compete_corpus::LEAP_CHECK, "module.exports=y=>y%4===0&&(y%100!==0||y%400===0);"),
];
const REQUESTS: [(&str, &str, &str); 10] = [
    ("A", "Change the value alpha to beta in every file item-00.txt through item-19.txt. Preserve the other line in each file. Make the edits with the available tools.", "edited"),
    ("B", "Scan shards 0, 1, 2 and 3 of the log using the available tools. Sum the units from the PAID entries only and answer with that integer alone.", "28"),
    ("C", "Use the three tools to run the project's test, lint and build checks. Answer only with the name of the check that failed, without the bench_ prefix.", "build"),
    ("F", "Implement lib.js so that module.exports is a function that turns a string into a lowercase slug: runs of non-alphanumeric characters become one hyphen, with no leading or trailing hyphen. check.js holds the tests. Write lib.js, run the check with the available tools, fix it until it passes, then answer done.", "done"),
    ("F", "Implement lib.js so that module.exports is a function that parses durations such as '1h30m', '45m' or '2h' into a number of seconds. check.js holds the tests. Write lib.js, run the check with the available tools, fix it until it passes, then answer done.", "done"),
    ("F", "Implement lib.js so that module.exports is a function that takes (x, lo, hi) and clamps x into the range from lo to hi. check.js holds the tests. Write lib.js, run the check with the available tools, fix it until it passes, then answer done.", "done"),
    ("F", "Implement lib.js so that module.exports is a function that removes duplicates from an array while keeping the order of first occurrence. check.js holds the tests. Write lib.js, run the check with the available tools, fix it until it passes, then answer done.", "done"),
    ("F", "Implement lib.js so that module.exports is a function that converts a positive integer to Roman numerals in subtractive form, for example 4 is IV and 1994 is MCMXCIV. check.js holds the tests. Write lib.js, run the check with the available tools, fix it until it passes, then answer done.", "done"),
    ("F", "Implement lib.js so that module.exports is a function that returns true when a year is a Gregorian leap year and false otherwise. check.js holds the tests. Write lib.js, run the check with the available tools, fix it until it passes, then answer done.", "done"),
    ("D", "In the GitHub repository acme/web, find the open issue labelled regression. Its body names a source file. Read that file with the filesystem tools and answer with the name of the function it exports, alone.", "formatInstant"),
];

// Suite D: a synthetic catalogue shaped like the official MCP servers in plan §4.
// Only the GitHub issue list and the filesystem read return task data.
const MCP: &[(&str, &str, &str)] = &[
    (
        "github_create_issue",
        "Create a new issue in a GitHub repository.",
        "owner,repo,title,body,labels",
    ),
    (
        "github_list_issues",
        "List issues in a GitHub repository with optional filters for state and labels.",
        "owner,repo,state,labels",
    ),
    (
        "github_get_issue",
        "Get the details of one issue in a GitHub repository.",
        "owner,repo,issue_number",
    ),
    (
        "github_update_issue",
        "Update the title, body, state or labels of an existing issue.",
        "owner,repo,issue_number,title,body,state",
    ),
    (
        "github_add_issue_comment",
        "Add a comment to an existing issue.",
        "owner,repo,issue_number,body",
    ),
    (
        "github_search_issues",
        "Search issues and pull requests across GitHub repositories.",
        "q,sort,order",
    ),
    (
        "github_create_pull_request",
        "Create a new pull request.",
        "owner,repo,title,head,base,body",
    ),
    (
        "github_list_pull_requests",
        "List and filter repository pull requests.",
        "owner,repo,state,base",
    ),
    (
        "github_get_pull_request",
        "Get the details of a specific pull request.",
        "owner,repo,pull_number",
    ),
    (
        "github_merge_pull_request",
        "Merge a pull request.",
        "owner,repo,pull_number,merge_method",
    ),
    (
        "github_get_pull_request_files",
        "List the files changed in a pull request.",
        "owner,repo,pull_number",
    ),
    (
        "github_create_branch",
        "Create a new branch in a GitHub repository.",
        "owner,repo,branch,from_branch",
    ),
    (
        "github_list_branches",
        "List the branches in a GitHub repository.",
        "owner,repo",
    ),
    (
        "github_list_commits",
        "List the commits of a branch in a GitHub repository.",
        "owner,repo,sha",
    ),
    (
        "github_get_commit",
        "Get the details of one commit.",
        "owner,repo,sha",
    ),
    (
        "github_search_code",
        "Search for code across GitHub repositories.",
        "q",
    ),
    (
        "github_search_repositories",
        "Search for GitHub repositories.",
        "query",
    ),
    (
        "github_get_file_contents",
        "Get the contents of a file or directory from a GitHub repository.",
        "owner,repo,path,branch",
    ),
    (
        "github_create_or_update_file",
        "Create or update a single file in a GitHub repository.",
        "owner,repo,path,content,message,branch",
    ),
    (
        "github_push_files",
        "Push several files to a GitHub repository in one commit.",
        "owner,repo,branch,files,message",
    ),
    (
        "github_fork_repository",
        "Fork a GitHub repository to your account.",
        "owner,repo",
    ),
    (
        "github_list_workflow_runs",
        "List the workflow runs of a repository.",
        "owner,repo,workflow_id",
    ),
    (
        "github_get_workflow_run_logs",
        "Download the logs of a workflow run.",
        "owner,repo,run_id",
    ),
    (
        "github_list_releases",
        "List the releases of a repository.",
        "owner,repo",
    ),
    (
        "github_get_me",
        "Get details of the authenticated GitHub user.",
        "",
    ),
    (
        "github_list_notifications",
        "List notifications for the authenticated user.",
        "all,participating",
    ),
    (
        "github_create_repository",
        "Create a new GitHub repository in your account.",
        "name,description,private",
    ),
    (
        "github_list_tags",
        "List the git tags of a repository.",
        "owner,repo",
    ),
    (
        "filesystem_read_file",
        "Read the complete contents of a file from the file system.",
        "path",
    ),
    (
        "filesystem_read_multiple_files",
        "Read the contents of multiple files at once.",
        "paths",
    ),
    (
        "filesystem_write_file",
        "Create a new file or overwrite an existing file.",
        "path,content",
    ),
    (
        "filesystem_edit_file",
        "Make line-based edits to a text file.",
        "path,edits,dryRun",
    ),
    (
        "filesystem_create_directory",
        "Create a new directory or ensure it exists.",
        "path",
    ),
    (
        "filesystem_list_directory",
        "List the files and directories in a path.",
        "path",
    ),
    (
        "filesystem_directory_tree",
        "Get a recursive tree view of files and directories.",
        "path",
    ),
    (
        "filesystem_move_file",
        "Move or rename files and directories.",
        "source,destination",
    ),
    (
        "filesystem_search_files",
        "Recursively search for files and directories matching a pattern.",
        "path,pattern,excludePatterns",
    ),
    (
        "filesystem_get_file_info",
        "Retrieve metadata about a file or directory.",
        "path",
    ),
    (
        "filesystem_list_allowed_directories",
        "List the directories this server may access.",
        "",
    ),
    ("browser_navigate", "Navigate the browser to a URL.", "url"),
    ("browser_navigate_back", "Go back to the previous page.", ""),
    (
        "browser_click",
        "Click an element on the page.",
        "element,ref,button",
    ),
    (
        "browser_type",
        "Type text into an editable element.",
        "element,ref,text,submit",
    ),
    (
        "browser_fill_form",
        "Fill several form fields at once.",
        "fields",
    ),
    (
        "browser_snapshot",
        "Capture an accessibility snapshot of the current page.",
        "",
    ),
    (
        "browser_take_screenshot",
        "Take a screenshot of the current page.",
        "filename,fullPage",
    ),
    (
        "browser_wait_for",
        "Wait for text to appear or disappear, or for a time to pass.",
        "text,textGone,time",
    ),
    ("browser_press_key", "Press a key on the keyboard.", "key"),
    (
        "browser_select_option",
        "Select an option in a dropdown.",
        "element,ref,values",
    ),
    (
        "browser_hover",
        "Hover over an element on the page.",
        "element,ref",
    ),
    (
        "browser_drag",
        "Drag and drop between two elements.",
        "startElement,startRef,endElement,endRef",
    ),
    (
        "browser_evaluate",
        "Evaluate a JavaScript expression on the page.",
        "function,element,ref",
    ),
    (
        "browser_console_messages",
        "Return all console messages of the page.",
        "",
    ),
    (
        "browser_network_requests",
        "Return all network requests since loading the page.",
        "",
    ),
    (
        "browser_tabs",
        "List, create, close or select a browser tab.",
        "action,index",
    ),
    ("browser_close", "Close the page.", ""),
    (
        "browser_resize",
        "Resize the browser window.",
        "width,height",
    ),
    ("browser_file_upload", "Upload one or more files.", "paths"),
    (
        "browser_handle_dialog",
        "Handle a dialog.",
        "accept,promptText",
    ),
    (
        "browser_install",
        "Install the browser named in the configuration.",
        "",
    ),
    (
        "sqlite_read_query",
        "Execute a SELECT query on the SQLite database.",
        "query",
    ),
    (
        "sqlite_write_query",
        "Execute an INSERT, UPDATE or DELETE query.",
        "query",
    ),
    (
        "sqlite_create_table",
        "Create a new table in the SQLite database.",
        "query",
    ),
    (
        "sqlite_list_tables",
        "List all tables in the SQLite database.",
        "",
    ),
    (
        "sqlite_describe_table",
        "Get the schema of a table.",
        "table_name",
    ),
    (
        "sqlite_append_insight",
        "Add a business insight to the memo.",
        "insight",
    ),
    (
        "memory_create_entities",
        "Create multiple new entities in the knowledge graph.",
        "entities",
    ),
    (
        "memory_create_relations",
        "Create multiple new relations between entities.",
        "relations",
    ),
    (
        "memory_add_observations",
        "Add new observations to existing entities.",
        "observations",
    ),
    (
        "memory_delete_entities",
        "Delete entities and their relations.",
        "entityNames",
    ),
    (
        "memory_delete_observations",
        "Delete specific observations from entities.",
        "deletions",
    ),
    (
        "memory_delete_relations",
        "Delete relations from the knowledge graph.",
        "relations",
    ),
    ("memory_read_graph", "Read the entire knowledge graph.", ""),
    (
        "memory_search_nodes",
        "Search for nodes in the knowledge graph.",
        "query",
    ),
    ("memory_open_nodes", "Open specific nodes by name.", "names"),
    (
        "fetch_fetch",
        "Fetch a URL and return its contents as markdown.",
        "url,max_length,start_index,raw",
    ),
    ("git_status", "Show the working tree status.", "repo_path"),
    (
        "git_diff_unstaged",
        "Show changes in the working directory that are not staged.",
        "repo_path,context_lines",
    ),
    (
        "git_diff_staged",
        "Show changes that are staged for commit.",
        "repo_path,context_lines",
    ),
    (
        "git_diff",
        "Show differences between branches or commits.",
        "repo_path,target,context_lines",
    ),
    (
        "git_commit",
        "Record changes to the repository.",
        "repo_path,message",
    ),
    (
        "git_add",
        "Add file contents to the staging area.",
        "repo_path,files",
    ),
    ("git_reset", "Unstage all staged changes.", "repo_path"),
    ("git_log", "Show the commit logs.", "repo_path,max_count"),
    (
        "git_create_branch",
        "Create a new branch from an optional base branch.",
        "repo_path,branch_name,base_branch",
    ),
    ("git_checkout", "Switch branches.", "repo_path,branch_name"),
    (
        "git_show",
        "Show the contents of a commit.",
        "repo_path,revision",
    ),
    ("git_branch", "List git branches.", "repo_path,branch_type"),
];
const ISSUES: &str = r#"[{"number":41,"title":"Docs typo on pricing page","labels":["docs"],"body":"The word 'monthly' is misspelled."},{"number":57,"title":"Dates render one day early","labels":["regression","bug"],"body":"Since the timezone refactor, the crash comes from src/date/format.ts."},{"number":63,"title":"Add dark mode toggle","labels":["enhancement"],"body":"Requested by several users."}]"#;
const SOURCE: &str = "import { zone } from './zone';\n\nexport function formatInstant(value: Date): string {\n  return zone(value).toISOString().slice(0, 10);\n}\n";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'a> {
    tier: &'a str,
    suite: &'a str,
    model: &'a str,
    effort: &'a str,
    seed: u32,
    arm: &'a str,
    native_tools: bool,
    input_tokens: usize,
    output_tokens: usize,
    reasoning_tokens_est: Option<usize>,
    ttft_ms: Option<u64>,
    token_source: &'a str,
    max_request_bytes: usize,
    max_input_tokens: usize,
    rounds: usize,
    host_calls: usize,
    tool_errors: usize,
    script_errors: usize,
    code_fallback: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter_trace: Option<Value>,
    script_ms: u64,
    code_metrics: crate::execution::CodeMetrics,
    wall_ms: u64,
    success: bool,
    partial: f64,
    cost_usd: u64,
    error: Option<String>,
}

struct Model<'a> {
    http: reqwest::Client,
    base: reqwest::Url,
    id: &'a str,
    effort: &'a str,
    native: bool,
    offline: bool,
}

async fn response(model: &Model<'_>, wire: &str) -> Result<(Value, Option<u64>), String> {
    let started = Instant::now();
    let mut stream = model
        .http
        .post(model.base.join("api/chat").unwrap())
        .body(wire.to_owned())
        .header("content-type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes_stream();
    let mut buffer = Vec::new();
    let mut text = String::new();
    let mut thinking = String::new();
    let mut calls = Vec::new();
    let mut ttft = None;
    while let Some(chunk) = stream.next().await {
        buffer.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        if buffer.len() > 2 * 1024 * 1024 {
            return Err("Ollama stream frame exceeds 2 MiB".into());
        }
        while let Some(newline) = buffer.iter().position(|&b| b == b'\n') {
            let line = buffer.drain(..=newline).collect::<Vec<_>>();
            let mut frame: Value = serde_json::from_slice(&line).map_err(|e| e.to_string())?;
            local_model(model.id, &frame)?;
            if let Some(error) = frame["error"].as_str() {
                return Err(error.into());
            }
            let delta = frame["message"]["content"].as_str().unwrap_or_default();
            let reasoning = frame["message"]["thinking"].as_str().unwrap_or_default();
            if !model.offline && (!delta.is_empty() || !reasoning.is_empty()) {
                ttft.get_or_insert_with(|| started.elapsed().as_millis() as u64);
            }
            text.push_str(delta);
            thinking.push_str(reasoning);
            if let Some(batch) = frame["message"]["tool_calls"].as_array() {
                calls.extend(batch.iter().cloned());
            }
            if frame["done"] == true {
                frame["message"] = json!({"role":"assistant","content":text,"thinking":thinking,"tool_calls":calls});
                return Ok((frame, ttft));
            }
        }
    }
    Err("Ollama stream ended before completion".into())
}

fn definition(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required}})
}

fn f_task(prompt: &str) -> (&'static str, &'static str) {
    let index = REQUESTS
        .iter()
        .filter(|case| case.0 == "F")
        .position(|case| case.1 == prompt)
        .expect("suite F prompt");
    F_TASKS[index]
}

fn definitions(suite: &str) -> Vec<Value> {
    match suite {
        "F" => api_tools::definitions(AccessMode::FileEdits)
            .into_iter()
            .filter(|d| d["name"] == READ || d["name"] == WRITE)
            .chain([definition(
                CHECK,
                "Run node check.js and return its output.",
                json!({}),
                &[],
            )])
            .collect(),
        "A" => api_tools::definitions(AccessMode::FileEdits)
            .into_iter()
            .filter(|d| d["name"] == READ || d["name"] == WRITE)
            .collect(),
        "D" => MCP
            .iter()
            .map(|(name, about, keys)| {
                let properties = keys
                    .split(',')
                    .filter(|key| !key.is_empty())
                    .map(|key| (key.to_owned(), json!({"type":"string"})))
                    .collect::<serde_json::Map<_, _>>();
                definition(name, about, Value::Object(properties), &[])
            })
            .collect(),
        "B" => vec![definition(
            SCAN,
            "Read a bounded log window from one of four shards. Returns log text.",
            json!({"shard":{"type":"integer"}}),
            &["shard"],
        )],
        _ => CHECKS
            .iter()
            .map(|name| {
                definition(
                    name,
                    &format!("Run the {} check.", &name[6..]),
                    json!({}),
                    &[],
                )
            })
            .collect(),
    }
}

fn prepare(root: &Path, suite: &str, prompt: &str) {
    if suite == "F" {
        fs::write(root.join("check.js"), f_task(prompt).0).unwrap();
        fs::write(
            root.join("lib.js"),
            "module.exports=()=>{throw new Error('todo')};\n",
        )
        .unwrap();
    }
    if suite == "A" {
        for i in 0..FILES {
            fs::write(
                root.join(format!("item-{i:02}.txt")),
                format!("value=alpha\nkeep={i:02}\n"),
            )
            .unwrap();
        }
    }
    if suite == "B" {
        let mut file = File::create(root.join("fixture.log")).unwrap();
        file.set_len(4 * WINDOW).unwrap();
        for shard in 0..4 {
            file.seek(SeekFrom::Start(shard * WINDOW)).unwrap();
            for i in 0..30 {
                writeln!(file, "INFO event={shard:02}{i:02} state=WAIT units=99").unwrap();
            }
            writeln!(
                file,
                "INFO event={shard:02}30 state=PAID units={}",
                4 + shard * 2
            )
            .unwrap();
        }
    }
    if suite == "C" {
        for check in CHECKS {
            fs::write(
                root.join(format!("{check}.cjs")),
                format!(
                    "console.log('check {}'); process.exitCode = {};\n",
                    &check[6..],
                    u8::from(check == "bench_build")
                ),
            )
            .unwrap();
        }
    }
}

async fn dispatch(input: &Invocation<'_>, suite: &str, name: &str, args: &Value) -> Value {
    match (suite, name) {
        ("A", READ | WRITE) => {
            let path = args["path"].as_str().unwrap_or_default();
            if !(0..FILES).any(|i| path == format!("item-{i:02}.txt")) {
                return code_mode::result(
                    "Use a fixture filename item-00.txt through item-19.txt",
                    true,
                );
            }
            api_tools::call(input, name, args, &mut |_| {}).await
        }
        ("B", SCAN) => {
            let Some(shard) = args["shard"].as_u64().filter(|&n| n < 4) else {
                return code_mode::result("Choose an integer shard from 0 to 3", true);
            };
            let read = || -> std::io::Result<String> {
                let mut file = File::open(input.workspace.join("fixture.log"))?;
                file.seek(SeekFrom::Start(shard * WINDOW))?;
                let mut window = String::new();
                file.take(2048).read_to_string(&mut window)?;
                window.truncate(window.find('\0').unwrap_or(window.len()));
                Ok(window)
            };
            match read() {
                Ok(text) => code_mode::result(&text, false),
                Err(error) => code_mode::result(&error.to_string(), true),
            }
        }
        ("C", name) if CHECKS.contains(&name) => {
            // Model output can select a fixture check, but never supplies command text.
            let command = format!("node {name}.cjs");
            api_tools::call(
                input,
                "lumen_run_command",
                &json!({"command":command}),
                &mut |_| {},
            )
            .await
        }
        ("F", READ | WRITE) => {
            let path = args["path"]
                .as_str()
                .unwrap_or_default()
                .trim_start_matches("./");
            if path != "lib.js" && !(name == READ && path == "check.js") {
                return code_mode::result("Only lib.js can be written; check.js can be read", true);
            }
            api_tools::call(
                input,
                name,
                &json!({"path": path, "content": args["content"]}),
                &mut |_| {},
            )
            .await
        }
        ("F", CHECK) => {
            api_tools::call(
                input,
                "lumen_run_command",
                &json!({"command":"node check.js"}),
                &mut |_| {},
            )
            .await
        }
        ("D", "github_list_issues") => {
            let args = args.to_string();
            if args.contains("acme") && args.contains("web") {
                code_mode::result(ISSUES, false)
            } else {
                code_mode::result("Repository not found", true)
            }
        }
        ("D", "filesystem_read_file") => {
            let path = args["path"].as_str().unwrap_or_default();
            if path
                .trim_start_matches("./")
                .ends_with("src/date/format.ts")
            {
                code_mode::result(SOURCE, false)
            } else {
                code_mode::result(&format!("ENOENT: no such file {path}"), true)
            }
        }
        ("D", name) if MCP.iter().any(|tool| tool.0 == name) => {
            code_mode::result("No results for this request", false)
        }
        _ => code_mode::result("Tool is outside this benchmark task", true),
    }
}

fn function_tool(definition: &Value) -> Value {
    json!({"type":"function","function":{"name":definition["name"],"description":definition["description"],"parameters":definition["inputSchema"]}})
}

async fn trial<'a>(
    model: &'a Model<'a>,
    case: (&'a str, &'a str, &'a str),
    cm: bool,
    seed: u32,
) -> Row<'a> {
    let (suite, prompt, expected) = case;
    let fixture = Fixture::new();
    let workspace = fixture.directory("project");
    let config = fixture.directory("accounts/bench");
    prepare(&workspace, suite, prompt);
    let account = Account {
        id: "bench".into(),
        name: "bench".into(),
        provider: Provider::Ollama,
    };
    let input = Invocation {
        rlm: None,
        automatic: false,
        account: &account,
        config: &config,
        workspace: &workspace,
        prompt,
        model: Some(model.id),
        effort: model.effort,
        access: AccessMode::Autopilot,
        approvals: None,
        resume: None,
        browser_scope: None,
        browser_artifacts: None,
        browser: None,
    };
    let (_cancel, stop) = watch::channel(false);
    let mut sandbox = None;
    let definitions = definitions(suite);
    let classic_tools = definitions.iter().map(function_tool).collect::<Vec<_>>();
    let code_tools = vec![function_tool(&code_mode::definition(&definitions))];
    let mut mode = code_mode::OllamaMode::new(cm);
    let mut filter_trace = Vec::new();
    let mut messages = vec![json!({"role":"user","content":prompt})];
    if !model.native {
        messages.insert(0, json!({"role":"system","content":""}));
    }
    let started = Instant::now();
    let mut row = Row {
        tier: if model.offline { "T0" } else { "T1" },
        suite,
        model: model.id,
        effort: model.effort,
        seed,
        arm: if cm { "CM" } else { "TC" },
        native_tools: model.native,
        input_tokens: 0,
        output_tokens: 0,
        reasoning_tokens_est: None,
        ttft_ms: None,
        token_source: if model.offline {
            "tiktoken-o200k-wire-estimate"
        } else {
            "ollama"
        },
        max_request_bytes: 0,
        max_input_tokens: 0,
        rounds: 0,
        host_calls: 0,
        tool_errors: 0,
        script_errors: 0,
        code_fallback: false,
        filter_trace: None,
        script_ms: 0,
        code_metrics: Default::default(),
        wall_ms: 0,
        success: false,
        partial: 0.,
        cost_usd: 0,
        error: None,
    };
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let errors = std::sync::atomic::AtomicUsize::new(0);
    let covered = std::sync::atomic::AtomicUsize::new(0);
    let invoke = |name: String, args: Value| {
        let (input, calls, errors, covered) = (&input, &calls, &errors, &covered);
        async move {
            use std::sync::atomic::Ordering::Relaxed;
            calls.fetch_add(1, Relaxed);
            let result = dispatch(input, suite, &name, &args).await;
            let expected_failure = suite == "C"
                && name == "bench_build"
                && result["content"][0]["text"]
                    .as_str()
                    .is_some_and(|s| s.contains("check build"));
            if result["isError"] == true && !expected_failure {
                errors.fetch_add(1, Relaxed);
            } else if suite == "B" && name == SCAN {
                covered.fetch_or(1 << args["shard"].as_u64().unwrap(), Relaxed);
            } else if suite == "D" {
                match name.as_str() {
                    "github_list_issues" => covered.fetch_or(1, Relaxed),
                    "filesystem_read_file" => covered.fetch_or(2, Relaxed),
                    _ => 0,
                };
            } else if suite == "C" {
                if let Some(index) = CHECKS.iter().position(|&check| check == name) {
                    covered.fetch_or(1 << index, Relaxed);
                }
            }
            result
        }
    };
    let mut trace = crate::execution::Trace::new("benchmark");
    let outcome: Result<String, String> = async {
        // The same bounded benchmark budget applies to both arms.
        for _ in 0..12 {
            let code = mode.active;
            let tools = if code { &code_tools } else { &classic_tools };
            if !model.native {
                let action = if code { "Return action=code and code=<script>." }
                    else { "Return action=tool, tool=<name>, arguments=<object>, and text empty." };
                messages[0]["content"] = json!(format!(
                    "{action} When finished, return action=final and your answer in text. Tools: {}", json!(tools)
                ));
            }
            let mut request = json!({"model":model.id,"messages":messages,"stream":true,
                "keep_alive":"10m","options":{"temperature":0,"seed":seed,"num_ctx":8192,"num_predict":768}});
            if model.effort != "default" {
                request["think"] = json!(model.effort);
            }
            if model.native {
                request["tools"] = json!(tools);
            } else {
                request["format"] = crate::ollama_agent::schema(tools, code);
            }
            let wire = request.to_string();
            row.max_request_bytes = row.max_request_bytes.max(wire.len());
            row.rounds += 1;
            let (response, ttft) = response(model, &wire).await?;
            if row.rounds == 1 {
                row.ttft_ms = ttft;
            }
            let (input_tokens, output_tokens) = if model.offline {
                let bpe = tiktoken_rs::o200k_base_singleton();
                (bpe.encode_ordinary(&wire).len(), bpe.encode_ordinary(&response["message"].to_string()).len())
            } else {
                (response["prompt_eval_count"].as_u64().ok_or("missing Ollama input usage")? as usize,
                 response["eval_count"].as_u64().ok_or("missing Ollama output usage")? as usize)
            };
            row.input_tokens += input_tokens;
            row.output_tokens += output_tokens;
            row.max_input_tokens = row.max_input_tokens.max(input_tokens);
            let message = &response["message"];
            if let Some(thinking) = message["thinking"].as_str().filter(|s| !s.is_empty()) {
                let count = tiktoken_rs::o200k_base_singleton().encode_ordinary(thinking).len();
                *row.reasoning_tokens_est.get_or_insert(0) += count;
            }
            let text = message["content"].as_str().unwrap_or_default();
            let requested = if model.native {
                message["tool_calls"].as_array().cloned().unwrap_or_default()
            } else {
                let action: Value = serde_json::from_str(text).map_err(|_| "invalid structured Ollama action")?;
                if action["action"] == "final" {
                    return Ok(action["text"].as_str().unwrap_or_default().into());
                }
                vec![match action["action"].as_str() {
                    Some("code") if code => native_call(code_mode::TOOL, json!({"code":action["code"]})),
                    Some("tool") if !code => native_call(action["tool"].as_str().unwrap_or_default(), action["arguments"].clone()),
                    _ => return Err("unknown structured Ollama action".into()),
                }]
            };
            if requested.is_empty() { return Ok(text.into()); }
            if requested.len() > 16 { return Err("Ollama native per-step tool cap exceeded".into()); }
            if model.native {
                messages.push(json!({"role":"assistant","content":text,"tool_calls":requested}));
            } else {
                messages.push(json!({"role":"assistant","content":text}));
            }
            for call in requested {
                let name = call["function"]["name"].as_str().unwrap_or_default();
                let args = &call["function"]["arguments"];
                let direct = (code && mode.active && name != code_mode::TOOL)
                    .then(|| code_mode::direct_script(name, args, &definitions))
                    .flatten();
                let result = if code && mode.active && (name == code_mode::TOOL || direct.is_some()) {
                    if sandbox.is_none() {
                        sandbox = Some(Session::new(stop.clone())?);
                    }
                    let script = direct.as_deref().or(args["code"].as_str()).ok_or("missing code")?;
                    let (mut result, syntax, failed) = sandbox.as_ref().unwrap()
                        .bench_run(script, &definitions, &invoke, &mut trace)
                        .await;
                    row.script_errors += usize::from(failed);
                    if let Some(activity) = mode.observe(syntax, &mut result) {
                        sandbox = None;
                        trace.observe(&activity);
                    }
                    if suite == "B" && !model.offline {
                        filter_trace.push(json!({"code": args["code"], "result": result["content"][0]["text"]}));
                    }
                    result
                } else if !code && name != code_mode::TOOL {
                    invoke(name.into(), args.clone()).await
                } else {
                    row.tool_errors += 1;
                    code_mode::result("Tool is outside the advertised arm", true)
                };
                messages.push(json!({"role":if model.native {"tool"} else {"user"},
                    "tool_name": name,"content":result.to_string()}));
            }
        }
        Err("benchmark step cap reached".into())
    }.await;
    row.code_metrics = trace.code_metrics();
    row.script_ms = row.code_metrics.script_ms;
    row.code_fallback = row.code_metrics.fallback;
    row.host_calls = calls.into_inner();
    row.tool_errors += errors.into_inner();
    row.wall_ms = started.elapsed().as_millis() as u64;
    if suite == "F" {
        let passed = std::process::Command::new("node")
            .arg("check.js")
            .current_dir(&workspace)
            .output()
            .is_ok_and(|out| out.status.success());
        row.partial = f64::from(passed);
    } else if suite == "A" {
        row.partial = (0..FILES)
            .filter(|&i| {
                fs::read_to_string(workspace.join(format!("item-{i:02}.txt"))).ok()
                    == Some(format!("value=beta\nkeep={i:02}\n"))
            })
            .count() as f64
            / FILES as f64;
    } else {
        let mask = match suite {
            "B" => 0b1111,
            "D" => 0b11,
            _ => 0b111,
        };
        let correct = outcome.as_ref().is_ok_and(|text| text.trim() == expected);
        row.partial = f64::from(correct && covered.into_inner() == mask);
    }
    row.success = row.partial == 1. && outcome.is_ok();
    if suite == "B" && !model.offline {
        row.filter_trace = Some(json!({"scripts": filter_trace, "answer": outcome.as_ref().ok()}));
    }
    row.error = outcome.err();
    row
}

fn native_call(name: &str, args: Value) -> Value {
    json!({"function":{"name":name,"arguments":args}})
}

async fn replay(axum::Json(request): axum::Json<Value>) -> String {
    let messages = request["messages"].as_array().unwrap();
    let content = messages[0]["content"].as_str().unwrap();
    let case = REQUESTS
        .iter()
        .find(|&&(_, prompt, _)| prompt == content)
        .unwrap();
    let (suite, prompt, _) = *case;
    let round = messages.iter().filter(|m| m["role"] == "assistant").count();
    let cm = request["tools"][0]["function"]["name"] == code_mode::TOOL;
    let calls = if cm && round == 0 {
        let script = match suite {
            "A" => {
                r#"(async () => {
                for (let i = 0; i < 20; i++) {
                    const path = 'item-' + String(i).padStart(2, '0') + '.txt';
                    const file = await tools.lumen_read_file({path});
                    await tools.lumen_write_file({path, content: file.content[0].text.replace('alpha', 'beta')});
                }
                return 'edited';
            })()"#
            }
            "B" => {
                r#"(async () => {
                const logs = await Promise.all([0,1,2,3].map(shard => tools.bench_scan_log({shard})));
                return logs.reduce((sum, row) => sum + Number(row.content[0].text.match(/state=PAID units=(\d+)/)[1]), 0);
            })()"#
            }
            "F" => &format!(
                "(async () => {{ await tools.lumen_write_file({{path:'lib.js', content:{}}}); const r = await tools.bench_check(); return r.isError ? r.text : 'done'; }})()",
                json!(f_task(prompt).1)
            ),
            "D" => {
                r#"(async () => {
                const issues = JSON.parse((await tools.github_list_issues({owner:'acme', repo:'web', labels:'regression'})).text);
                const path = issues.find(i => i.labels.includes('regression')).body.match(/src\/\S+\.ts/)[0];
                const source = (await tools.filesystem_read_file({path})).text;
                return source.match(/export function (\w+)/)[1];
            })()"#
            }
            _ => {
                r#"(async () => {
                const names = ['test', 'lint', 'build'];
                const results = await Promise.all(names.map(name => tools['bench_' + name]()));
                return names[results.findIndex(result => result.isError)];
            })()"#
            }
        };
        vec![native_call(code_mode::TOOL, json!({"code":script}))]
    } else if !cm {
        match (suite, round) {
            ("A", 0..=3) => {
                let start = if round % 2 == 0 { 0 } else { 16 };
                (start..(start + 16).min(FILES))
                    .map(|i| {
                        let path = format!("item-{i:02}.txt");
                        if round < 2 {
                            native_call(READ, json!({"path":path}))
                        } else {
                            let text = messages
                                .iter()
                                .filter(|m| m["role"] == "tool")
                                .nth(i)
                                .and_then(|m| result_text(m).map(|(_, text)| text))
                                .unwrap_or_default()
                                .replace("alpha", "beta");
                            native_call(WRITE, json!({"path":path,"content":text}))
                        }
                    })
                    .collect()
            }
            ("B", 0) => (0..4)
                .map(|i| native_call(SCAN, json!({"shard":i})))
                .collect(),
            ("C", 0) => CHECKS
                .iter()
                .map(|name| native_call(name, json!({})))
                .collect(),
            ("F", 0) => vec![native_call(
                WRITE,
                json!({"path":"lib.js","content":f_task(prompt).1}),
            )],
            ("F", 1) => vec![native_call(CHECK, json!({}))],
            ("D", 0) => vec![native_call(
                "github_list_issues",
                json!({"owner":"acme","repo":"web","labels":"regression"}),
            )],
            ("D", 1) => vec![native_call(
                "filesystem_read_file",
                json!({"path":"src/date/format.ts"}),
            )],
            _ => vec![],
        }
    } else {
        vec![]
    };
    let message = if calls.is_empty() {
        let results = messages
            .iter()
            .filter(|m| m["role"] == "tool")
            .filter_map(|m| {
                result_text(m)
                    .map(|(error, text)| (error, text, m["tool_name"].as_str().unwrap_or("")))
            })
            .collect::<Vec<_>>();
        let answer = if cm {
            results
                .last()
                .map(|(_, text, _)| text.clone())
                .unwrap_or_default()
        } else {
            match suite {
                "B" => results
                    .iter()
                    .filter_map(|(_, text, _)| {
                        text.lines().find_map(|line| {
                            line.split_once("state=PAID units=")?.1.parse::<u32>().ok()
                        })
                    })
                    .sum::<u32>()
                    .to_string(),
                "F" => "done".into(),
                "D" => results
                    .last()
                    .and_then(|(_, text, _)| {
                        text.split_once("export function ")?
                            .1
                            .split('(')
                            .next()
                            .map(str::to_owned)
                    })
                    .unwrap_or_default(),
                "C" => results
                    .iter()
                    .find(|(error, _, _)| *error)
                    .map(|(_, _, name)| name.trim_start_matches("bench_").to_owned())
                    .unwrap_or_default(),
                _ => "edited".into(),
            }
        };
        json!({"role":"assistant","content":answer})
    } else {
        json!({"role":"assistant","content":"","tool_calls":calls})
    };
    format!("{}\n", json!({"message":message,"done":true}))
}

fn result_text(message: &Value) -> Option<(bool, String)> {
    let result: Value = serde_json::from_str(message["content"].as_str()?).ok()?;
    Some((
        result["isError"] == true,
        result["content"][0]["text"].as_str()?.into(),
    ))
}

fn save(row: &impl Serialize) {
    let line = serde_json::to_string(row).unwrap();
    println!("{line}");
    if let Some(path) = std::env::var_os("LUMEN_BENCH_OUTPUT") {
        let mut file = File::options()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{line}").unwrap();
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap()
}

// T2: official APIs through the product client, so every request reserves budget in
// api.sqlite first. The ledger is a fresh benchmark directory, not the user's data:
// LUMEN_BENCH_CAP_USD becomes each account's budget, capping the whole run.
fn cached_tokens(provider: &Provider, response: &Value) -> u64 {
    let usage = match provider {
        Provider::Anthropicapi => &response["usage"]["cache_read_input_tokens"],
        Provider::Openaiapi => &response["usage"]["input_tokens_details"]["cached_tokens"],
        _ => &response["usageMetadata"]["cachedContentTokenCount"],
    };
    usage.as_u64().unwrap_or(0)
}

async fn trial_api(
    data: &Path,
    account: &Account,
    model: &str,
    case: (&'static str, &'static str, &'static str),
    cm: bool,
    seed: u32,
) -> Value {
    let (suite, prompt, expected) = case;
    let fixture = Fixture::new();
    let workspace = fixture.directory("project");
    let config = fixture.directory("accounts/bench");
    prepare(&workspace, suite, prompt);
    let input = Invocation {
        rlm: None,
        automatic: false,
        account,
        config: &config,
        workspace: &workspace,
        prompt,
        model: Some(model),
        effort: "",
        access: AccessMode::Autopilot,
        approvals: None,
        resume: None,
        browser_scope: None,
        browser_artifacts: None,
        browser: None,
    };
    let definitions = definitions(suite);
    let tools = if cm {
        vec![code_mode::definition(&definitions)]
    } else {
        definitions.clone()
    };
    let client = crate::api::Client::new(data, account, model, false);
    let (_cancel, stop) = watch::channel(false);
    let sandbox = Session::new(stop).unwrap();
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let invoke = |name: String, args: Value| {
        let (input, calls) = (&input, &calls);
        async move {
            calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            dispatch(input, suite, &name, &args).await
        }
    };
    let mut trace = crate::execution::Trace::new("benchmark");
    let (mut tokens_in, mut tokens_out, mut cached, mut rounds, mut errors, mut micros) =
        (0u64, 0u64, 0u64, 0, 0, 0f64);
    let started = Instant::now();
    let outcome: Result<String, String> = async {
        let client = client?;
        let mut history = crate::api_wire::initial(&account.provider, prompt);
        for _ in 0..12 {
            rounds += 1;
            let (response, turn) = client
                .step(
                    &history,
                    &tools,
                    Some(768),
                    |event: crate::models::Activity| {
                        if event.kind == "api-cost" {
                            let cost: Value = serde_json::from_str(&event.text).unwrap_or_default();
                            if cost["status"] == "reported" {
                                micros += cost["estimatedUsd"].as_f64().unwrap_or(0.) * 1e6;
                            }
                        }
                    },
                )
                .await?;
            let (i, o) = turn.usage.ok_or("API reported no usage")?;
            tokens_in += i;
            tokens_out += o;
            cached += cached_tokens(&account.provider, &response);
            if let Some(error) = turn.error {
                return Err(error);
            }
            crate::api_wire::append_assistant(&account.provider, &mut history, &response);
            if turn.calls.is_empty() {
                return Ok(turn.text.trim().to_owned());
            }
            let mut results = vec![];
            for call in turn.calls {
                let direct = (cm && call.name != code_mode::TOOL)
                    .then(|| code_mode::direct_script(&call.name, &call.args, &definitions))
                    .flatten();
                let result = if cm && (call.name == code_mode::TOOL || direct.is_some()) {
                    let code = direct.unwrap_or_else(|| {
                        call.args["code"].as_str().unwrap_or_default().to_owned()
                    });
                    let (result, _, failed) = sandbox
                        .bench_run(&code, &definitions, &invoke, &mut trace)
                        .await;
                    errors += usize::from(failed);
                    result
                } else if !cm && call.name != code_mode::TOOL {
                    invoke(call.name.clone(), call.args.clone()).await
                } else {
                    errors += 1;
                    code_mode::result("Tool is outside the advertised arm", true)
                };
                results.push((call, result));
            }
            crate::api_wire::append_results(&account.provider, &mut history, results);
        }
        Err("benchmark step cap reached".into())
    }
    .await;
    let passed = match suite {
        "A" => (0..FILES).all(|i| {
            fs::read_to_string(workspace.join(format!("item-{i:02}.txt"))).ok()
                == Some(format!("value=beta\nkeep={i:02}\n"))
        }),
        "F" => std::process::Command::new("node")
            .arg("check.js")
            .current_dir(&workspace)
            .output()
            .is_ok_and(|out| out.status.success()),
        _ => outcome.as_ref().is_ok_and(|text| text.trim() == expected),
    };
    json!({"tier":"T2","suite":suite,"model":model,"provider":format!("{:?}", account.provider),
        "seed":seed,"arm":if cm {"CM"} else {"TC"},"inputTokens":tokens_in,"outputTokens":tokens_out,
        "cachedInputTokens":cached,"rounds":rounds,"hostCalls":calls.into_inner(),"scriptErrors":errors,
        "codeMetrics":trace.code_metrics(),"wallMs":started.elapsed().as_millis() as u64,
        "costUsd":micros / 1e6,"success":passed && outcome.is_ok(),"answer":outcome.as_ref().ok(),
        "error":outcome.err()})
}

/// LUMEN_BENCH_API: comma-separated provider:accountId:model:inputUsd:outputUsd entries,
/// using official per-million rates. Keys are read from the OS keychain by account ID.
#[tokio::test]
#[ignore = "spends real API credit within LUMEN_BENCH_CAP_USD; run only with owner approval"]
async fn bench_t2() {
    let cap: f64 = std::env::var("LUMEN_BENCH_CAP_USD")
        .expect("set LUMEN_BENCH_CAP_USD")
        .parse()
        .unwrap();
    assert!(cap > 0. && cap <= 100., "cap must be between 0 and 100 USD");
    let data = std::path::PathBuf::from(
        std::env::var("LUMEN_BENCH_DATA")
            .expect("set LUMEN_BENCH_DATA to an empty benchmark directory"),
    );
    fs::create_dir_all(&data).unwrap();
    let specs = std::env::var("LUMEN_BENCH_API").expect("set LUMEN_BENCH_API");
    let seeds: u32 = std::env::var("LUMEN_BENCH_SEEDS")
        .unwrap_or_else(|_| "3".into())
        .parse()
        .unwrap();
    let suites = std::env::var("LUMEN_BENCH_SUITES").unwrap_or_else(|_| "A,B,C,D,F".into());
    for spec in specs.split(',') {
        let [provider, id, model, input, output] = spec.split(':').collect::<Vec<_>>()[..] else {
            panic!("bad LUMEN_BENCH_API entry {spec}");
        };
        let provider = match provider {
            "anthropic" => Provider::Anthropicapi,
            "openai" => Provider::Openaiapi,
            "google" => Provider::Googleapi,
            other => panic!("unknown provider {other}"),
        };
        let account = Account {
            id: id.into(),
            name: "bench".into(),
            provider,
        };
        crate::api::bench_configure(
            &data,
            id,
            model,
            input.parse().unwrap(),
            output.parse().unwrap(),
            cap,
        )
        .unwrap();
        for seed in 0..seeds {
            for case in REQUESTS
                .iter()
                .filter(|case| suites.split(',').any(|s| s == case.0))
            {
                let first =
                    (seed ^ (case.0.as_bytes()[0] as u32)).wrapping_mul(0x45d9f3b) >> 29 & 1 == 0;
                for cm in [first, !first] {
                    let row = trial_api(&data, &account, model, *case, cm, seed).await;
                    let exhausted = row["error"]
                        .as_str()
                        .is_some_and(|e| e.contains("budget exhausted"));
                    save(&row);
                    if exhausted {
                        println!("Benchmark cap reached; stopping.");
                        return;
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn bench_t0() {
    let address = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = endpoint(&address.local_addr().unwrap().to_string()).unwrap();
    let app = axum::Router::new().route("/api/chat", axum::routing::post(replay));
    let server = tokio::spawn(async move { axum::serve(address, app).await.unwrap() });
    let model = Model {
        http: client(),
        base,
        id: "scripted",
        effort: "default",
        native: true,
        offline: true,
    };
    for case in REQUESTS {
        let tc = trial(&model, case, false, 0).await;
        let cm = trial(&model, case, true, 0).await;
        save(&tc);
        save(&cm);
        assert!(tc.success && cm.success);
        assert_eq!(tc.host_calls, cm.host_calls);
        assert_eq!(tc.code_metrics.host_calls, 0);
        assert_eq!(cm.code_metrics.host_calls, cm.host_calls as u64);
        assert!(cm.code_metrics.bytes_returned > 0);
        let (classic, code) = match case.0 {
            "A" => (5, 2),
            "D" | "F" => (3, 2),
            _ => (2, 2),
        };
        assert_eq!((tc.rounds, cm.rounds), (classic, code));
        if matches!(case.0, "B" | "D") {
            assert!(cm.max_request_bytes * 2 < tc.max_request_bytes);
        }
    }
    server.abort();
}

type LongCase = (
    &'static str,
    &'static str,
    &'static str,
    Vec<(String, String)>,
);

fn long_cases() -> Vec<LongCase> {
    use std::fmt::Write as _;
    let noise = "Archive background. A routine entry had no recorded change. This explanatory note is unrelated to the records under study.\n";
    let padding = noise.repeat(9500);
    let mut aggregate = String::new();
    for (index, n) in [2, 4, 7, 8, 3, 9, 5, 6].into_iter().enumerate() {
        aggregate.push_str(&padding[..padding.len() / 8]);
        writeln!(
            aggregate,
            "\nEVENT region={} units={n}",
            if index % 2 == 0 { "north" } else { "south" }
        )
        .unwrap();
    }
    let mut links = String::new();
    for row in [
        "START K71",
        "LINK K71 -> K42",
        "LINK K42 -> K16",
        "VALUE K16 = vermilion",
    ] {
        links.push_str(&padding[..padding.len() / 4]);
        writeln!(links, "\n{row}").unwrap();
    }
    let repo = [
        ("api_tools.rs", include_str!("api_tools.rs")),
        ("api_wire.rs", include_str!("api_wire.rs")),
        ("code_mode.rs", include_str!("code_mode.rs")),
        ("ollama_agent.rs", include_str!("ollama_agent.rs")),
    ]
    .into_iter()
    .map(|(name, text)| (name.into(), text.into()))
    .chain([(String::from("historical-build-notes.md"), padding.clone())])
    .collect();
    let corpus = [
        (
            "archive",
            format!("{padding}\nNOTE: Bayside used the old retention code amber."),
        ),
        (
            "policy",
            format!("{padding}\nCURRENT POLICY: Bayside uses the stone retention code."),
        ),
    ]
    .into_iter()
    .map(|(name, text)| (name.into(), text))
    .collect();
    // OOLONG-like: the label needs reading, not a regex, and the tickets exceed the window.
    let issues = [
        (
            "S",
            "The courier left my parcel at an address two streets away",
        ),
        ("B", "I was charged twice for the same order last Tuesday"),
        (
            "A",
            "I cannot reset my password because the reset page loops back",
        ),
        (
            "S",
            "The tracking page has not moved in nine days and the box is still at the depot",
        ),
        (
            "B",
            "The refund you promised three weeks ago never reached my card",
        ),
        (
            "S",
            "The box arrived crushed and two of the plates inside were broken",
        ),
        (
            "A",
            "After changing my email address I am locked out of my profile",
        ),
        ("B", "The invoice lists the wrong VAT number for my company"),
        (
            "S",
            "The van driver marked the package as handed over but nobody rang the bell",
        ),
        (
            "A",
            "My profile shows somebody else's name and order history",
        ),
        (
            "B",
            "My card was declined at checkout yet the money left my account",
        ),
        (
            "S",
            "The carrier sent the parcel back to your warehouse without trying again",
        ),
    ];
    let filler = "I have been a customer for several years and usually everything is fine. I would appreciate a reply when someone has time to look into it. Thank you for reading this message. ";
    let mut tickets = String::new();
    for n in 0..36 {
        let (_, issue) = issues[(n * 5) % issues.len()];
        tickets.push_str(&padding[..padding.len() / 64]);
        writeln!(tickets, "\nTICKET {}: {filler}{issue}. {filler}", n + 1).unwrap();
    }
    let shipping = (0..36)
        .filter(|n| issues[(n * 5) % issues.len()].0 == "S")
        .count()
        .to_string();
    let shipping: &'static str = Box::leak(shipping.into_boxed_str());
    vec![
        ("classification", "Each TICKET line in the support log describes one customer problem. How many tickets are about a delivery or parcel problem (not billing or account access)? Reply with the integer only.", shipping,
            vec![("support-log".into(), tickets)]),
        ("aggregation", "In the event archive, sum units from EVENT lines where region=north. Reply with the integer only.", "17",
            vec![("event-archive".into(), aggregate)]),
        ("multi-hop", "Find the key on the START line in the link log. Follow each LINK until reaching its VALUE. Reply with the value only.", "vermilion",
            vec![("link-log".into(), links)]),
        ("repository", "In api_tools.rs, what is the maximum project file size for lumen_read_file, as an integer number of bytes? Reply with the integer alone.", "2000000", repo),
        ("document-qa", "What is the current Bayside retention code? Use the CURRENT POLICY line, and reply with the code word alone.", "stone", corpus),
    ]
}

#[tokio::test]
#[ignore = "uses an installed loopback Ollama model; no cloud endpoints"]
async fn bench_e() {
    use crate::rlm::{Effort, Sources, Turn};
    use std::sync::atomic::Ordering::Relaxed;
    let address = std::env::var("LUMEN_BENCH_URL").expect("set LUMEN_BENCH_URL");
    let base = endpoint(&address).unwrap();
    let model_id = std::env::var("LUMEN_BENCH_MODELS").expect("set one LUMEN_BENCH_MODELS ID");
    let model = Model {
        http: client(),
        base,
        id: &model_id,
        effort: "default",
        native: true,
        offline: false,
    };
    let show = crate::rlm::bounded_json(
        crate::ollama_agent::post(
            &model.http,
            &model.base,
            "api/show",
            &json!({"model":model_id}),
        )
        .await
        .unwrap(),
    )
    .await
    .unwrap();
    local_model(model.id, &show).unwrap();
    assert!(show["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c == "tools"));
    let fixture = Fixture::new();
    let grep = definition("lumen_grep_context",
        "Search line by line with a regular expression in a document. Returns matching {line,text} rows.",
        json!({"id":{"type":"integer"},"pattern":{"type":"string"},"limit":{"type":"integer"}}),
        &["id","pattern"]);
    let slice = definition(
        "lumen_slice_context",
        "Read a document range by UTF-8 byte offsets.",
        json!({"id":{"type":"integer"},"start":{"type":"integer"},"end":{"type":"integer"}}),
        &["id", "start", "end"],
    );
    let classic = [function_tool(&grep), function_tool(&slice)];
    let seeds = std::env::var("LUMEN_BENCH_SEEDS")
        .unwrap_or_else(|_| "5".into())
        .parse::<usize>()
        .unwrap();
    let only = std::env::var("LUMEN_BENCH_CASE").ok();
    for (index, (case, task, expected, documents)) in long_cases().into_iter().enumerate() {
        if only.as_deref().is_some_and(|only| only != case) {
            continue;
        }
        let mut sources = Sources::default();
        let mut size = 0;
        for (name, text) in documents {
            size += tiktoken_rs::o200k_base_singleton()
                .encode_ordinary(&text)
                .len();
            sources.add(&name, text);
        }
        assert!(size > 100_000, "{case}: {size}");
        let catalog = Turn::new(
            &fixture.root,
            &address,
            Some(model.id),
            Effort::Low,
            &sources,
        )
        .document(&json!({"method":"list"}))
        .unwrap();
        let prompt = format!(
            "{task}\nDocuments are available by numeric ID: {catalog}. Search the source evidence before answering. If llm() is available, use it to check the relevant excerpts before giving the final answer.",
        );
        for seed in 0..seeds {
            let arms = ["TC", "CM", "CM+RLM"];
            for step in 0..3 {
                let arm = arms[(index + seed + step) % arms.len()];
                let effort = if arm == "CM+RLM" {
                    Effort::Medium
                } else {
                    Effort::Low
                };
                let (_stop, cancel) = watch::channel(false);
                let sandbox = Session::new(cancel).unwrap().with_rlm(Turn::new(
                    &fixture.root,
                    &address,
                    Some(model.id),
                    effort,
                    &sources,
                ));
                let turn = sandbox.rlm.as_ref().unwrap();
                let code = [function_tool(&code_mode::definition_with_context(
                    &[],
                    effort,
                ))];
                let tools: &[Value] = if arm == "TC" { &classic } else { &code };
                let mut history = vec![json!({"role":"user","content":prompt})];
                let (mut input, mut output, mut rounds, mut errors) = (0, 0, 0, 0);
                let (mut peak, mut ttft) = (0, None);
                let mut trace = crate::execution::Trace::new("benchmark");
                let mut scripts = Vec::new();
                let start = Instant::now();
                let outcome: Result<String, String> = tokio::time::timeout(Duration::from_secs(45), async {
                    for _ in 0..4 {
                        rounds += 1;
                        let request = json!({"model":model.id,"messages":history,"tools":tools,"stream":true,"keep_alive":"10m",
                            "options":{"temperature":0,"seed":seed,"num_ctx":8192,"num_predict":320}});
                        let (response, latency) = response(&model, &request.to_string()).await?;
                        if rounds == 1 { ttft = latency; }
                        let i = response["prompt_eval_count"].as_u64().ok_or("Ollama supplied no input usage")?;
                        input += i;
                        peak = peak.max(i);
                        output += response["eval_count"].as_u64().ok_or("Ollama supplied no output usage")?;
                        let msg = &response["message"];
                        let calls = msg["tool_calls"].as_array().unwrap();
                        if calls.is_empty() {
                            return Ok(msg["content"].as_str().unwrap_or_default().trim().into());
                        }
                        if calls.len() > 16 { return Err("Ollama native per-step tool cap exceeded".into()); }
                        history.push(json!({"role":"assistant","content":msg["content"],"tool_calls":calls}));
                        for call in calls {
                            let name = call["function"]["name"].as_str().unwrap_or_default();
                            let mut args = call["function"]["arguments"].clone();
                            let result = match (arm, name) {
                                ("TC", "lumen_grep_context" | "lumen_slice_context") if args.is_object() => {
                                    args["method"] = json!(if name == "lumen_grep_context" {"grep"} else {"slice"});
                                    match turn.document(&args) {
                                        Ok(value) => code_mode::result(&value.to_string(), false),
                                        Err(e) => code_mode::result(&e, true),
                                    }
                                }
                                ("CM" | "CM+RLM", code_mode::TOOL) => {
                                    let result = sandbox.bench_run(args["code"].as_str().ok_or("missing code")?, &[], |_, _| async {
                                        code_mode::result("Tool is outside this benchmark catalog", true)
                                    }, &mut trace).await.0;
                                    scripts.push(json!({"code": args["code"], "result": result["content"][0]["text"].as_str().map(|t| t.chars().take(600).collect::<String>())}));
                                    result
                                }
                                _ => code_mode::result("Tool is outside the advertised arm", true),
                            };
                            errors += usize::from(result["isError"] == true);
                            let mut text = result.to_string();
                            crate::wire::bound("code-output", &mut text);
                            history.push(json!({"role":"tool","tool_name":name,"content":text}));
                        }
                    }
                    Err("Benchmark step cap reached".into())
                }).await.unwrap_or_else(|_| Err("Benchmark 45-second trial limit reached".into()));
                let metrics = trace.code_metrics();
                save(
                    &json!({"tier":"T1","suite":"E","case":case,"model":model.id,
                    "arm":arm,"seed":seed,"effort":if effort==Effort::Low {"low"} else {"medium"},
                    "corpusTokens":size,"inputTokens":input,"outputTokens":output,"rounds":rounds,
                    "subRounds":metrics.sub_calls,"subInputTokens":turn.usage.input.load(Relaxed),
                    "subOutputTokens":turn.usage.output.load(Relaxed),"peakRootInputTokens":peak,"ttftMs":ttft,"scriptMs":metrics.script_ms,
                    "codeMetrics":metrics,
                    "toolErrors":errors,"wallMs":start.elapsed().as_millis(),"costUsd":0,
                    "success":outcome.as_ref().is_ok_and(|text| text.eq_ignore_ascii_case(expected)),
                    "answer":outcome.as_ref().ok(),"expected":expected,"error":outcome.err(),"scripts":scripts}),
                );
            }
        }
    }
}

#[tokio::test]
#[ignore = "uses an installed loopback Ollama model; no cloud endpoints"]
async fn live_rlm_query() {
    use crate::rlm::{Effort, Sources, Turn};
    use std::sync::atomic::Ordering::Relaxed;
    let model = std::env::var("LUMEN_BENCH_MODELS").expect("set one local model ID");
    let address = std::env::var("LUMEN_BENCH_URL").expect("set LUMEN_BENCH_URL");
    let fixture = Fixture::new();
    let mut sources = Sources::default();
    let (_, _, expected, documents) = long_cases().remove(1);
    for (name, text) in documents {
        sources.add(&name, text);
    }
    let (_stop, cancel) = watch::channel(false);
    let sandbox = Session::new(cancel).unwrap().with_rlm(Turn::new(
        &fixture.root,
        &address,
        Some(&model),
        Effort::Medium,
        &sources,
    ));
    let output = sandbox.run(
        r#"(async () => {
            const records = (await context.grep(0, '^EVENT region=north')).map(row => row.text).join('\n');
            return await llm('Sum the units in these records. Reply with only the integer.\n' + records,
                {effort:'low', maxTokens:128});
        })()"#,
        &[],
        |_, _| async { panic!("Unexpected workspace tool in the subquery fixture") },
    ).await.unwrap();
    let usage = &sandbox.rlm.as_ref().unwrap().usage;
    save(
        &json!({"tier":"T1","suite":"E","case":"direct-subquery-smoke","model":model,
        "costUsd":0,"subRounds":usage.rounds.load(Relaxed),"subInputTokens":usage.input.load(Relaxed),
        "subOutputTokens":usage.output.load(Relaxed),"answer":output,"expected":expected,
        "success":output.trim()==expected}),
    );
    assert_eq!(usage.rounds.load(Relaxed), 1);
    assert_eq!(output.trim(), expected);
}

#[tokio::test]
#[ignore = "uses installed local Ollama models; no cloud endpoints"]
async fn bench_t1() {
    let base = endpoint(&std::env::var("LUMEN_BENCH_URL").expect("set LUMEN_BENCH_URL")).unwrap();
    let ids = std::env::var("LUMEN_BENCH_MODELS").expect("set LUMEN_BENCH_MODELS");
    let suite = std::env::var("LUMEN_BENCH_SUITE").ok();
    assert!(suite
        .as_deref()
        .is_none_or(|suite| REQUESTS.iter().any(|case| case.0 == suite)));
    for id in ids.split(',') {
        let http = client();
        local_model(id, &Value::Null).unwrap();
        let show: Value = http
            .post(base.join("api/show").unwrap())
            .json(&json!({"model":id}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        local_model(id, &show).unwrap();
        let supported = |name: &str| {
            show["capabilities"]
                .as_array()
                .is_some_and(|list| list.iter().any(|capability| capability == name))
        };
        let effort = std::env::var("LUMEN_BENCH_EFFORT").unwrap_or_else(|_| "default".into());
        assert!(matches!(
            effort.as_str(),
            "default" | "low" | "medium" | "high"
        ));
        assert!(effort == "default" || supported("thinking"));
        let model = Model {
            http,
            base: base.clone(),
            id,
            effort: &effort,
            native: supported("tools"),
            offline: false,
        };
        let seeds = std::env::var("LUMEN_BENCH_SEEDS")
            .unwrap_or_else(|_| "5".into())
            .parse::<u32>()
            .unwrap();
        for seed in 0..seeds {
            for case in REQUESTS {
                if suite.as_deref().is_some_and(|suite| suite != case.0) {
                    continue;
                }
                // Interleave paired arms in reproducible pseudorandom order.
                let first =
                    (seed ^ (case.0.as_bytes()[0] as u32)).wrapping_mul(0x45d9f3b) >> 29 & 1 == 0;
                for cm in [first, !first] {
                    save(&trial(&model, case, cm, seed).await);
                }
            }
        }
    }
}
