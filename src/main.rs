extern crate ctrlc;

use std::{
    env,
    sync::{
        Mutex, Arc
    }
};

use rusqlite::{
    params,
    Connection
};

type SqliteResult<T> = rusqlite::Result<T>;

use serenity::{
    prelude::*,
    model::{
        channel::{Message, Reaction},
        gateway::{Ready, Activity},
        id::{UserId, GuildId, MessageId}
    },
    framework::standard::{
        StandardFramework, CommandResult,
        macros::{command, group}
    }
};

#[group]
#[commands(ping)]
struct UserManagement;

struct Handler {
    db: Arc<Mutex<Connection>>
}

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} ({}) is connected!", ready.user.name, ready.user.id);
        ctx.set_activity(Activity::playing("with cat toys"));
    }

    fn reaction_add(&self, ctx: Context, rxn: Reaction) {
        let guild_id = match rxn.guild_id {
            Some(x) => x,
            None => return
        };

        let result = {
            let db_lock = self.db.lock().unwrap();

            let signed_guild_id = guild_id.0 as i64;

            let mut stmt = db_lock.prepare("SELECT message_id FROM guild_configs WHERE guild_id = ?1").unwrap();
            stmt.query_row(params![signed_guild_id], |row| -> SqliteResult<MessageId> {
                row.get::<_, i64>(0).map(|x: i64| MessageId(x as u64))
            })
        };

        if let Ok(x) = result {
            if x == rxn.message_id {
                let msg_fn: Box<dyn Fn(&str)> = match rxn.user_id.create_dm_channel(&ctx) {
                    Err(x) => {
                        println!("Error {} while creating DM channel for user {}.", x, rxn.user_id);
                        Box::from(|_: &str| {})
                    },
                    Ok(channel) => {
                        let ctx_c = ctx.clone();
                        let user_id_c = rxn.user_id.clone();
                        Box::from(move |msg: &str| {
                            if let Err(x) = channel.say(&ctx_c, msg) {
                                println!("Error {} while sending message into DM with user {}.", x, user_id_c);
                            }
                        })
                    }
                };
                assign_guest(ctx, rxn.user_id, guild_id, self.db.clone(), msg_fn);
            }
        }
    }
}

fn invalid_command(ctx: &mut Context, msg: &Message, cmd: &str) {
    if let Err(_) = msg.channel_id.say(&ctx, format!("Command `{}` not found.", cmd)) {
        println!("Couldn't send reply in {}.", msg.channel_id);
    }
}

fn assign_guest(_ctx: Context, _uid: UserId, _gid: GuildId, _db: Arc<Mutex<Connection>>, _f: impl Fn(&str)) {
    _f("Hello there ya fucker!");
}

fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Error when reading token!");
    let db_file = env::var("DB_FILE").expect("Error while getting database file!");

    let mut client = {
        let db_orig = Arc::from(Mutex::from(Connection::open(db_file).expect("Error while opening database!")));
        Client::new(&token, Handler {db: db_orig}).expect("Error while creating client!")
    };

    client.with_framework(StandardFramework::new()
        .configure(|c| c.prefix("?"))
        .group(&USERMANAGEMENT_GROUP)
        .unrecognised_command(invalid_command)
    );

    let shard_manager_c = Arc::clone(&client.shard_manager);
    ctrlc::set_handler(move || {
        shard_manager_c.lock().shutdown_all();
    }).expect("Error while setting SIGINT handler!");

    if let Err(msg) = client.start() {
        println!("Error occured on client: {:?}", msg);
    }
}

#[command]
fn ping(ctx: &mut Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(ctx, "Pong!")?;

    Ok(())
}