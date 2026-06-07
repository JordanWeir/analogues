use loco_rs::prelude::*;
use rig::client::{CompletionClient, ProviderClient};
use rig::completion::Prompt;
use rig::providers::{anthropic, openrouter};
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
        let _anthropic_client =
            anthropic::Client::from_env().expect("Can generate anthropic client from env");
        let openrouter_client =
            openrouter::Client::from_env().expect("Can generate openrouter client from env");

        /*
           Current ultra-cheap model intelligence hierarchy from Artificial Analysis: https://artificialanalysis.ai/#intelligence
           - Deepseek v4 Flash, $112.86, 47
           - MiMo-V2.5-Pro, $160.82, 54
           - MiniMax-M3, $306.79, 55
           - Haiku 4.5, $619.69, 37
        */

        // Create agent with a single context prompt
        let comedian_agent = openrouter_client
            .agent("deepseek/deepseek-v4-flash")
            .preamble("You are a comedian here to entertain the user using humour and jokes.")
            .build();

        // Prompt the agent and print the response
        let response = comedian_agent
            .prompt("Entertain me!")
            .await
            .expect("We get a result from the api");

        println!("{response}");

        Ok(())
    }
}
