use std::env;

extern crate ctrlc;

use serenity::{
    prelude::*,
    model::{
        channel::Message, 
        gateway::{Ready, Activity}
    },
    framework::standard::{
        StandardFramework, CommandResult,
        macros::{command, group}
    }
};

#[group]
#[commands(ping)]
struct UserManagement;

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} ({}) is connected!", ready.user.name, ready.user.id);
        ctx.set_activity(Activity::playing("with cat toys"))
    }
}

fn invalid_command(ctx: &mut Context, msg: &Message, cmd: &str) {
    if let Err(_x) = msg.channel_id.say(ctx, format!("Command `{}` not found.", cmd)) {
        println!("Couldn't send reply in {}.", msg.channel_id);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN")?;

    let mut client = Client::new(&token, Handler)?;

    client.with_framework(StandardFramework::new()
        .configure(|c| c.prefix("?"))
        .group(&USERMANAGEMENT_GROUP)
        .unrecognised_command(invalid_command)
    );

    let shard_manager = client.shard_manager.clone();
    ctrlc::set_handler(move || {
        shard_manager.lock().shutdown_all();
    })?;

    if let Err(msg) = client.start() {
        println!("Error occured on client: {:?}", msg);
    }

    Ok(())
}

#[command]
fn ping(ctx: &mut Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(ctx, "Pong!")?;

    Ok(())
}