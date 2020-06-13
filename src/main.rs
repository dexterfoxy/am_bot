extern crate ctrlc;

mod unwrap_ext;

use crate::unwrap_ext::*;

use std::{
    env,
    sync::{
        Mutex, Arc
    },
    time::{
        Duration,
        SystemTime
    }
};

use rusqlite::{
    params,
    Connection
};

type SqliteResult<T> = rusqlite::Result<T>;
type SqliteError     = rusqlite::Error;

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

struct Handler;

static mut DB: Option<Mutex<Connection>> = None;

// Simple wrapper to avoid ugly unsafe blocks
#[inline(always)]
fn get_db() -> &'static Mutex<Connection> {
    unsafe {DB.unwrap_ref()} // Our own custom wrapper
}

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        eprintln!("{} ({}) is connected!", ready.user.name, ready.user.id);
        ctx.set_activity(Activity::playing("with cat toys")); // TODO: Database entry for this
    }

    fn reaction_add(&self, ctx: Context, rxn: Reaction) {
        let current_user = ctx.http.get_current_user().expect("Error while getting user info.");
        if current_user.id == rxn.user_id {
            return;
        }

        let guild_id = match rxn.guild_id {
            Some(x) => x,
            None => return
        };

        let s_gid = guild_id.0 as i64;

        let result = {
            let db_lock = get_db().lock().expect("Error while locking DB.");

            let mut stmt = db_lock.prepare("SELECT message_id FROM guild_configs WHERE guild_id = ?1").expect("Prepare failed.");
            stmt.query_row(params![s_gid], |row| -> SqliteResult<MessageId> {
                row.get::<_, i64>(0).map(|x: i64| MessageId(x as u64))
            })
        };

        if let Ok(x) = result {
            if x == rxn.message_id {
                if let Err(e) = rxn.delete(&ctx) {
                    eprintln!("Couldn't delete reaction: {:?}", e);
                }

                let msg_fn: Box<dyn Fn(&str)> = match rxn.user_id.create_dm_channel(&ctx) {
                    Err(x) => {
                        eprintln!("Error '{:?}' while creating DM channel for user {}.", x, rxn.user_id);
                        Box::from(|_: &str| {})
                    },
                    Ok(channel) => {
                        let http_c = ctx.http.clone();
                        let user_id_c = rxn.user_id.clone();
                        Box::from(move |msg: &str| {
                            if let Err(x) = channel.say(&http_c, msg) {
                                eprintln!("Error '{:?}' while sending message into DM with user {}.", x, user_id_c);
                            }
                        })
                    }
                }; // TODO: Get rid of the fuckery above and make a function for it

                assign_guest(ctx, rxn.user_id, guild_id, msg_fn);
            }
        }
    }
}

fn invalid_command(ctx: &mut Context, msg: &Message, cmd: &str) {
    if let Err(x) = msg.channel_id.say(&ctx, format!("Command `{}` not found.", cmd)) {
        eprintln!("Error '{:?}' while sending reply in {}.", x, msg.channel_id);
    }
}

fn assign_guest(_ctx: Context, uid: UserId, gid: GuildId, f: impl Fn(&str)) {
    let s_gid = gid.0 as i64;
    let s_uid = uid.0 as i64;

    println!("{} {}", s_gid, s_uid);

    let db_lock = get_db().lock().expect("Couldn't lock the database.");
    
    let result = {
        let mut stmt = db_lock.prepare("SELECT timestamp, expired FROM guests WHERE guild_id = ?1 AND user_id = ?2").expect("Error while preparing statement.");
        stmt.query_row(params![s_gid, s_uid], |row| -> SqliteResult<(SystemTime, bool)> {
            Ok((
                SystemTime::UNIX_EPOCH.checked_add(
                    Duration::from_secs(row.get::<_, i64>(0)? as u64)
                ).expect("Error while adding time."), 
                row.get::<_, i64>(1)? != 0
            ))
        })
    };

    if let Err(x) = result {
        if let SqliteError::QueryReturnedNoRows = x {

        } else {
            panic!("Error '{}' while reading database.", x);
        }
    } else {
        f("Sorry, but you cannot request guest access more than once on a single guild.");
    }
}

fn main() {
    // Environment variables to avoid hardcoding
    let token   = env::var("DISCORD_TOKEN").expect("Error while reading DISCORD_TOKEN! Set the environment variable.");
    let db_file = env::var("DB_FILE")      .expect("Error while reading DB_FILE! Set the environment variable.");

    // Unsafe block needed for assignment
    // Wrapper function above for easy mutex access
    unsafe {DB = Some(Mutex::from(Connection::open(db_file).expect("Error while opening database.")));} 
    
    // Token is never stored on disk
    let mut client = Client::new(&token, Handler {/* Nothing here at the moment */}).expect("Error while creating client.");

    client.with_framework(StandardFramework::new()
        .configure(|c| c.prefix("?")) // Possibly, a per-guild config would be ideal
        .group(&USERMANAGEMENT_GROUP)
        .unrecognised_command(invalid_command)
    );

    let shard_manager_c = Arc::clone(&client.shard_manager); // Moved out of scope by closure, no explicit mem::drop needed
    ctrlc::set_handler(move || {
        shard_manager_c.lock().shutdown_all();
    }).expect("Error while setting SIGINT handler.");

    if let Err(msg) = client.start() {
        eprintln!("Error '{:?}' occured on client.", msg);
    }
}

#[command]
fn ping(ctx: &mut Context, msg: &Message) -> CommandResult { // Testing command, really unnecessary
    msg.channel_id.say(ctx, "Pong!")?;

    Ok(())
}