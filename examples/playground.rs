use analogues::app::App;
#[allow(unused_imports)]
use loco_rs::{cli::playground, prelude::*};
use rig::client::{CompletionClient, ProviderClient};
use rig::completion::Prompt;
use rig::providers::{anthropic, openrouter};

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    let _ctx = playground::<App>().await?;

    // let active_model: articles::ActiveModel = articles::ActiveModel {
    //     title: Set(Some("how to build apps in 3 steps".to_string())),
    //     content: Set(Some("use Loco: https://loco.rs".to_string())),
    //     ..Default::default()
    // };
    // active_model.insert(&ctx.db).await.unwrap();

    // let res = articles::Entity::find().all(&ctx.db).await.unwrap();
    // println!("{:?}", res);
    // println!("welcome to playground. edit me at `examples/playground.rs`");

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
