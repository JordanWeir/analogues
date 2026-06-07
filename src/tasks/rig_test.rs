use loco_rs::prelude::*;
use rig::client::{CompletionClient, ProviderClient};
use rig::completion::Prompt;
use rig::providers::anthropic;
pub struct RigTest;
#[async_trait]
impl Task for RigTest {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "rigTest".to_string(),
            detail: "Task generator".to_string(),
        }
    }
    async fn run(&self, _app_context: &AppContext, _vars: &task::Vars) -> Result<()> {
        println!("Task RigTest generated");

        // Create Anthropic client
        let client = anthropic::Client::from_env().expect("Can generate anthropic client from env");

        // Create agent with a single context prompt
        let comedian_agent = client
            .agent("claude-haiku-4-5")
            .preamble("You are a comedian here to entertain the user using humour and jokes.")
            .build();

        // Prompt the agent and print the response
        let response = comedian_agent.prompt("Entertain me!").await.expect("We get a result from the api");

        println!("{response}");

        Ok(())
    }
}
