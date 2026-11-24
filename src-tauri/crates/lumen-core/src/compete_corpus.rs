//! Offline Compete corpus: small tasks with their own tests, each implemented by two
//! scripted candidates standing in for two providers. Every candidate runs in a real
//! Git worktree and is judged by the project's own check (`verify:` in LUMEN.md),
//! exactly as Compete does. Single applies candidate A, the agent Auto would pick
//! alone; Compete applies the winner. Applied results are checked again in the
//! project, so the comparison is measured, not assumed. No CLI, account or network.
use crate::{gitops, orchestra};
use std::path::{Path, PathBuf};

struct Task {
    name: &'static str,
    check: &'static str,
    a: &'static str,
    b: &'static str,
}

pub(crate) const SLUG_CHECK: &str = "const a=require('assert');const f=require('./lib.js');a.strictEqual(f('Hello, World!'),'hello-world');a.strictEqual(f('  A  b '),'a-b');";
pub(crate) const DURATION_CHECK: &str = "const a=require('assert');const f=require('./lib.js');a.strictEqual(f('1h30m'),5400);a.strictEqual(f('45m'),2700);a.strictEqual(f('2h'),7200);";
pub(crate) const CLAMP_CHECK: &str = "const a=require('assert');const f=require('./lib.js');a.strictEqual(f(5,0,3),3);a.strictEqual(f(-1,0,3),0);a.strictEqual(f(2,0,3),2);";
pub(crate) const DEDUPE_CHECK: &str = "const a=require('assert');const f=require('./lib.js');a.deepStrictEqual(f([3,1,3,2,1]),[3,1,2]);";
pub(crate) const ROMAN_CHECK: &str = "const a=require('assert');const f=require('./lib.js');a.strictEqual(f(4),'IV');a.strictEqual(f(1994),'MCMXCIV');";
pub(crate) const LEAP_CHECK: &str = "const a=require('assert');const f=require('./lib.js');a.strictEqual(f(2000),true);a.strictEqual(f(1900),false);a.strictEqual(f(2024),true);";

fn corpus() -> Vec<Task> {
    vec![
        Task {
            name: "slugify",
            check: SLUG_CHECK,
            a: "module.exports=s=>s.toLowerCase().replace(/[^a-z0-9]+/g,'-').replace(/^-|-$/g,'');",
            b: "module.exports=s=>s.toLowerCase().replace(/[^a-z0-9]+/g,'-');",
        },
        Task {
            name: "parse duration",
            check: DURATION_CHECK,
            a: "module.exports=s=>{const[h,m]=s.split('h');return h*3600+(parseInt(m)||0)*60;};",
            b: "module.exports=s=>{let t=0;for(const[,n,u]of s.matchAll(/(\\d+)([hm])/g))t+=n*(u==='h'?3600:60);return t;};",
        },
        Task {
            name: "clamp",
            check: CLAMP_CHECK,
            a: "module.exports=(x,lo,hi)=>Math.min(hi,Math.max(lo,x));",
            b: "module.exports=function(x,lo,hi){\n  if(x<lo){\n    return lo;\n  }\n  if(x>hi){\n    return hi;\n  }\n  return x;\n};",
        },
        Task {
            name: "dedupe keeping order",
            check: DEDUPE_CHECK,
            a: "module.exports=xs=>[...new Set(xs)];",
            b: "module.exports=xs=>[...new Set(xs)].sort();",
        },
        Task {
            name: "roman numerals",
            check: ROMAN_CHECK,
            a: "module.exports=n=>{let s='';for(const[v,r]of[[1000,'M'],[500,'D'],[100,'C'],[50,'L'],[10,'X'],[5,'V'],[1,'I']])while(n>=v){s+=r;n-=v;}return s;};",
            b: "module.exports=n=>{let s='';for(const[v,r]of[[1000,'M'],[900,'CM'],[500,'D'],[400,'CD'],[100,'C'],[90,'XC'],[50,'L'],[40,'XL'],[10,'X'],[9,'IX'],[5,'V'],[4,'IV'],[1,'I']])while(n>=v){s+=r;n-=v;}return s;};",
        },
        Task {
            name: "leap year",
            check: LEAP_CHECK,
            a: "module.exports=y=>y%4===0;",
            b: "module.exports=y=>y%4===0&&y%100!==0;",
        },
    ]
}

async fn repo(task: &Task) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(format!("lumen-compete-{}", crate::models::id()));
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::write(dir.join("LUMEN.md"), "verify: node check.js\n").ok()?;
    std::fs::write(dir.join("check.js"), task.check).ok()?;
    std::fs::write(
        dir.join("lib.js"),
        "module.exports=()=>{throw new Error('todo')};\n",
    )
    .ok()?;
    gitops::run(&dir, &["init", "-q"], None).await.ok()?;
    gitops::run(&dir, &["add", "."], None).await.ok()?;
    gitops::run(
        &dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "init",
        ],
        None,
    )
    .await
    .ok()?;
    dunce::canonicalize(dir).ok()
}

struct Outcome {
    single_passes: bool,
    compete_passes: bool,
    verdict: String,
}

async fn passes(dir: &Path) -> bool {
    let (_tx, cancel) = tokio::sync::watch::channel(false);
    matches!(orchestra::check_candidate(dir, "", cancel).await, Ok((p, 0, _)) if p > 0)
}

/// Runs one task through Compete's judging against real worktrees and checks.
async fn run(task: &Task) -> Option<Outcome> {
    let top = repo(task).await?;
    let base = gitops::write_tree(&top).await.ok()?;
    let (_tx, cancel) = tokio::sync::watch::channel(false);
    let mut cands = vec![];
    let mut patches = vec![];
    let mut lanes = vec![];
    for (label, agent, code) in [("A", "Claude", task.a), ("B", "GPT", task.b)] {
        let lane = top.with_file_name(format!("{}-{label}", top.file_name()?.to_string_lossy()));
        let branch = format!("lumen/compete-{label}-{}", crate::models::id());
        gitops::add_lane(&top, &base, &lane, &branch, "HEAD").await.ok()?;
        std::fs::write(lane.join("lib.js"), format!("{code}\n")).ok()?;
        let patch = gitops::lane_patch(&lane, &base).await.ok()?;
        let (passed, failed, failure) = orchestra::check_candidate(&lane, &patch, cancel.clone())
            .await
            .ok()?;
        cands.push(orchestra::Candidate {
            id: format!("Candidate {label}"),
            agent: agent.into(),
            ran: !patch.trim().is_empty(),
            passed,
            failed,
            failure,
            preferred: false,
            diff: orchestra::diff_size(&patch),
        });
        patches.push(patch);
        lanes.push((lane, branch));
    }
    let single_passes = cands[0].ran && cands[0].failed == 0 && cands[0].passed > 0;
    let winner = orchestra::pick_winner(&cands);
    let verdict = orchestra::compete_verdict(&cands, winner);
    if let Some(w) = winner {
        gitops::apply_patch(&top, &patches[w]).await.ok()?;
    }
    let compete_passes = passes(&top).await;
    for (lane, branch) in &lanes {
        gitops::remove_lane(&top, lane, branch).await;
    }
    let _ = std::fs::remove_dir_all(&top);
    Some(Outcome {
        single_passes,
        compete_passes,
        verdict,
    })
}

fn node() -> bool {
    std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[tokio::test]
async fn one_candidate_failing_its_checks_loses_and_the_passing_one_is_applied() {
    if !node() {
        return;
    }
    let task = &corpus()[1];
    let Some(o) = run(task).await else { return };
    assert!(!o.single_passes, "candidate A must fail its checks");
    assert!(
        o.compete_passes,
        "the applied winner must pass in the project"
    );
    assert!(
        o.verdict
            .starts_with("Candidate B (GPT) won: 1/1 checks passed"),
        "{}",
        o.verdict
    );
    assert!(
        o.verdict
            .contains("Candidate A (Claude) failed its checks (Project verify"),
        "{}",
        o.verdict
    );
}

#[tokio::test]
async fn compete_beats_single_on_the_offline_corpus() {
    if !node() {
        return;
    }
    let (mut single, mut compete, mut runs) = (0, 0, 0);
    let mut table =
        String::from("| Task | Single | Compete | Lumen's verdict |\n|---|---|---|---|\n");
    for task in corpus() {
        let Some(o) = run(&task).await else { return };
        runs += 1;
        single += usize::from(o.single_passes);
        compete += usize::from(o.compete_passes);
        // Compete never applies something worse than Single on the same candidates.
        assert!(
            o.compete_passes || !o.single_passes,
            "{}: {}",
            task.name,
            o.verdict
        );
        let mark = |b: bool| if b { "pass" } else { "fail" };
        table.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            task.name,
            mark(o.single_passes),
            mark(o.compete_passes),
            o.verdict
        ));
    }
    println!("{table}\nSingle: {single}/{runs} · Compete: {compete}/{runs} · worker runs: Single {runs}, Compete {}", runs * 2);
    assert_eq!((single, compete, runs), (3, 5, 6));
}
