use gh_workflow_tailcall::gh_workflow::toolchain::Toolchain;
use gh_workflow_tailcall::gh_workflow::{Cargo, Env, Job, Level, Permissions, Run, Step, Strategy};
use gh_workflow_tailcall::*;
use serde_json::json;

#[test]
fn generate() {
    let workflow: Workflow = Workflow::default().auto_fix(true);
    let mut workflow = workflow.to_ci_workflow();
    workflow = workflow.add_env(("FORGE_KEY", "${{ secrets.OPEN_ROUTER }}"));
    if let Some(mut jobs) = workflow.jobs {
        jobs.insert("build".to_string(), test_job());

        workflow.jobs = Some(jobs);
    }

    workflow.generate().unwrap();
}

fn test_job() -> Job {
    let cmd: Cargo = Cargo::new("test")
        .id("test")
        .args("--all-features --workspace")
        .name("Cargo Test");
    let mut step: Step<Run> = cmd.into();
    step = step.add_env(Env::new("FORGE_MODEL", "${{ matrix.model }}"));

    Job::new("Build and Test")
        .permissions(Permissions::default().contents(Level::Read))
        .add_step(Step::checkout())
        .add_step(Toolchain::default().add_stable())
        .strategy(Strategy::default().matrix(json!( { "model": ["google/gemini-2.0-flash-exp:free", "anthropic/claude-3.5-sonnet", "openchat/openchat-7b:free", "meta-llama/llama-3.1-70b-instruct:free"] } )))
        .add_step(step)
}
